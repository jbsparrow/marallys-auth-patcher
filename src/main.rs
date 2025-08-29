use io::Result as IoResult;
use std::path::Path;
use std::{
    env, fs,
    io::{self, BufRead, Write},
    path::PathBuf,
    process::{self, Stdio},
};

use base64::prelude::*;
use reqwest::header;
use reqwest::Result as ReqwestResult;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::MmcaiError;

mod errors;

pub type Result<T> = std::result::Result<T, MmcaiError>;

#[derive(Serialize)]
struct AuthRequest<'a> {
    login: &'a str,
    password: &'a str,
    #[serde(rename = "accessToken")]
    access_token: &'a str,
}



impl Default for AuthRequest<'_> {
    fn default() -> Self {
        AuthRequest {
            login: "herobrine",
            password: "",
            access_token: "null",
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Agent<'a> {
    name: &'a str,
    version: i32,
}
impl Default for Agent<'_> {
    fn default() -> Self {
        Agent {
            name: "Minecraft",
            version: 1,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthResponse {
    data: AuthData,
    status: String,
    status_code: u16,
    message: String,
    errors: Vec<String>,
}


#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct AuthData {
    uuid: String,
    name: String,
    access_token: String,
    expired_date: Option<String>, // optional since it could be null
    texture_skin_url: Option<String>,
    texture_cloak_url: Option<String>,
    texture_skin_guid: Option<String>,
    texture_cloak_guid: Option<String>,
    full_skin_url: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Profile {
    id: String,
    name: String,
}

#[derive(Debug)]
struct LoginResult {
    prefetched_data: String,
    access_token: String,
    selected_profile: Profile,
}

fn validate_args(args: &[String]) -> Result<()> {
    match args.len() {
        len if len < 4 => Err(MmcaiError::InvalidArgument(args[0].to_owned())),
        4 => Err(MmcaiError::CannotRunDirectly),
        _ => Ok(()),
    }
}

fn find_authlib_injector(path: Option<&Path>) -> Option<PathBuf> {
    let path = match path {
        Some(p) => p.to_path_buf(),
        None => {
            let exe_path = env::current_exe().ok()?;
            exe_path.parent()?.to_path_buf()
        }
    };

    let is_filename_valid =
        |filename: &str| filename.starts_with("authlib-injector") && filename.ends_with(".jar");

    fs::read_dir(path).ok().and_then(|entries| {
        entries
            .filter_map(IoResult::ok)
            .find(|entry| {
                let file_name = entry.file_name();
                file_name.to_str().map_or(false, is_filename_valid)
            })
            .map(|entry| entry.path())
    })
}

fn generate_client_token() -> String {
    Uuid::new_v4().to_string()
}

fn yggdrasil_login(
    username: &str,
    password: &str,
    client_token: &str,
    api_url: &str,
) -> Result<LoginResult> {
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(MmcaiError::ReqwestClientBuildFailed)?;

    let signin_url = api_url.replace("/authlib/minecraft", "/auth/signin");


    // 1. Fetch the metadata for -Dauthlibinjector.yggdrasil.prefetched
    let get_prefetched_data = || -> ReqwestResult<String> {
        let prefetched_data_text = client.get(api_url).send()?.text()?;
        Ok(BASE64_STANDARD.encode(prefetched_data_text))
    };

    // 2. Prepare headers
    let mut headers = header::HeaderMap::new();
    headers.insert("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:138.0) Gecko/20100101 Firefox/138.0".parse().unwrap());
    headers.insert("Accept", "application/json".parse().unwrap());
    headers.insert("Accept-Language", "en-US,en;q=0.5".parse().unwrap());
    headers.insert("Content-Type", "application/json".parse().unwrap());

    let auth_body = AuthRequest {
        login: username,
        password,
        access_token: "null",
    };

    // 3. Send POST /auth/signin request
    let perform_authentication = || -> ReqwestResult<AuthResponse> {
        client
            .post(&signin_url)
            .headers(headers.clone())
            .json(&auth_body)
            .send()?
            .json::<AuthResponse>()
    };

    let prefetched_data = get_prefetched_data().map_err(MmcaiError::YggdrasilHelloFailed)?;

    let auth_response = match perform_authentication() {
        Ok(resp) => resp,
        Err(source) => {
            let response = client
                .post(&signin_url)
                .headers(headers.clone())
                .json(&auth_body)
                .send();

            let response_body = match response {
                Ok(res) => res.text().unwrap_or_else(|_| "<failed to read response body>".into()),
                Err(_) => "<request failed, no response body>".into(),
            };

            return Err(MmcaiError::YggdrasilAuthFailed {
                source,
                response: response_body,
            });
        }
    };

    Ok(LoginResult {
        prefetched_data,
        access_token: auth_response.data.access_token.clone(),
        selected_profile: Profile {
            id: auth_response.data.uuid.clone(),
            name: auth_response.data.name.clone(),
        },
    })
}



fn modify_minecraft_params(
    minecraft_params: &mut [String],
    access_token: &str,
    uuid: &str,
    playername: &str,
) -> Result<()> {
    for index in 0..minecraft_params.len() {
        match minecraft_params[index].as_str() {
            line if line.contains("param --username") => {
                *minecraft_params
                    .get_mut(index + 1)
                    .ok_or(MmcaiError::Other)? = format!("param {}", playername).to_string();
            }
            line if line.contains("param --uuid") => {
                *minecraft_params
                    .get_mut(index + 1)
                    .ok_or(MmcaiError::Other)? = format!("param {}", uuid).to_string();
            }
            line if line.contains("param --accessToken") => {
                *minecraft_params
                    .get_mut(index + 1)
                    .ok_or(MmcaiError::Other)? = format!("param {}", access_token).to_string();
            }
            line if line.contains("userName ") => {
                *minecraft_params.get_mut(index).ok_or(MmcaiError::Other)? =
                    format!("userName {}", playername).to_string();
            }
            line if line.contains("sessionId ") => {
                *minecraft_params.get_mut(index).ok_or(MmcaiError::Other)? =
                    format!("sessionId token:{}", access_token).to_string();
            }
            _ => {}
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    validate_args(&args)?;

    // find authlib-injector
    let authlib_injector_path =
        find_authlib_injector(None).ok_or(MmcaiError::AuthlibInjectorNotFound)?;

    println!(
        "[mmcai_rs] authlib-injector found at {:?}, logging in...",
        authlib_injector_path
    );

    // yggdrasil part
    let username = &args[1];
    let password = &args[2];
    let api_url = &args[3];

    let client_token = generate_client_token();

    let login_result = yggdrasil_login(username, password, &client_token, api_url)?;

    println!(
        "[mmcai_rs] Successfully authenticated as {}",
        login_result.selected_profile.name
    );

    // minecraft params
    let mut minecraft_params: Vec<String> = Vec::new();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line
            .map_err(MmcaiError::ReadMinecraftParamsFailed)?
            .trim()
            .to_string();
        minecraft_params.push(line.clone());
        if line == "launch" {
            break;
        }
    }

    let access_token = login_result.access_token;
    let uuid = login_result.selected_profile.id;
    let playername = login_result.selected_profile.name;

    modify_minecraft_params(&mut minecraft_params, &access_token, &uuid, &playername)?;

    // ready to launch
    let java_executable = env::var("INST_JAVA").map_err(|_| MmcaiError::JavaExecutableNotFound)?;

    let mut jvm_args = Vec::from(&args[5..]);
    jvm_args.insert(
        0,
        format!(
            "-javaagent:{}={}",
            authlib_injector_path.to_str().ok_or(MmcaiError::Other)?,
            api_url
        ),
    );
    jvm_args.insert(
        1,
        format!(
            "-Dauthlibinjector.yggdrasil.prefetched={}",
            login_result.prefetched_data
        ),
    );

    #[cfg(debug_assertions)]
    {
        println!("[mmcai_rs] args: {:?}", args);
        println!("[mmcai_rs] java_executable: {:?}", java_executable);
        println!("[mmcai_rs] jvm_args: {:?}", jvm_args);
        println!("[mmcai_rs] minecraft_params: {:?}", minecraft_params);
    }

    let mut command = process::Command::new(java_executable);
    command.args(jvm_args);

    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .spawn()
        .map_err(MmcaiError::SpawnProcessFailed)?;

    let stdin = child.stdin.as_mut().ok_or(MmcaiError::StdinUnavailable)?;

    minecraft_params.iter().for_each(|line| {
        let _ = writeln!(stdin, "{}", line).map_err(MmcaiError::WriteMinecraftParamsFailed);
    });

    let status = child.wait().map_err(|_| MmcaiError::Other)?;

    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use assert_fs::prelude::{FileTouch, PathChild};
    use fake::{Fake, Faker};
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    use super::*;

    fn get_fake_args(length: usize) -> Vec<String> {
        let seed = [
            1, 0, 0, 0, 23, 0, 0, 0, 200, 1, 0, 0, 210, 30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0,
        ];
        let r = &mut StdRng::from_seed(seed);
        (0..length)
            .map(|_| Faker.fake_with_rng::<String, _>(r))
            .collect()
    }

    #[test]
    fn test_validate_args() {
        assert!(matches!(
            validate_args(&get_fake_args(1)),
            Err(MmcaiError::InvalidArgument(_))
        ));
        assert!(matches!(
            validate_args(&get_fake_args(2)),
            Err(MmcaiError::InvalidArgument(_))
        ));
        assert!(matches!(
            validate_args(&get_fake_args(3)),
            Err(MmcaiError::InvalidArgument(_))
        ));
        assert!(matches!(
            validate_args(&get_fake_args(4)),
            Err(MmcaiError::CannotRunDirectly)
        ));
        assert!(matches!(validate_args(&get_fake_args(5)), Ok(())));
    }

    #[test]
    fn test_generate_client_token() {
        let client_token = generate_client_token();
        assert_eq!(client_token.len(), 36);
    }

    #[test]
    fn test_find_authlib_injector() {
        let test_find_authlib_injector_with_filename = |filename: &str, should_exist: bool| {
            let temp_dir = assert_fs::TempDir::new().unwrap();
            let input_file = temp_dir.child(filename);
            input_file.touch().unwrap();
            if should_exist {
                assert_eq!(
                    find_authlib_injector(Some(&temp_dir)).unwrap(),
                    input_file.path()
                );
            } else {
                assert!(find_authlib_injector(Some(&temp_dir)).is_none());
            }
            temp_dir.close().unwrap();
        };

        test_find_authlib_injector_with_filename("authlib-injector-1.0.0.jar", true);
        test_find_authlib_injector_with_filename("authlib-injector-1.0.0.zip", false);
        test_find_authlib_injector_with_filename("authlib-injector-1.0.0", false);
        test_find_authlib_injector_with_filename("authlib-injector-.catch.me.if.you.can.jar", true);
        test_find_authlib_injector_with_filename("not-start-with.authlib-injector.jar", false);
        test_find_authlib_injector_with_filename("authlib-injector.jar.not-end-with", false);
    }

    #[test]
    fn test_modify_minecraft_params() {
        let mut minecraft_params = vec![
            "---START---".to_string(),
            "param --username".to_string(),
            "param AnyHow".to_string(),
            "param --uuid".to_string(),
            "param AnyHow".to_string(),
            "param --accessToken".to_string(),
            "param AnyHow".to_string(),
            "userName AnyHow".to_string(),
            "sessionId AnyHow".to_string(),
            "launch".to_string(),
            "---END---".to_string(),
        ];
        let access_token = "TEST_ACCESS_TOKEN";
        let uuid = "TEST_UUID";
        let playername = "TEST_PLAYERNAME";
        modify_minecraft_params(&mut minecraft_params, access_token, uuid, playername).unwrap();
        assert_eq!(
            minecraft_params,
            vec![
                "---START---".to_string(),
                "param --username".to_string(),
                "param TEST_PLAYERNAME".to_string(),
                "param --uuid".to_string(),
                "param TEST_UUID".to_string(),
                "param --accessToken".to_string(),
                "param TEST_ACCESS_TOKEN".to_string(),
                "userName TEST_PLAYERNAME".to_string(),
                "sessionId token:TEST_ACCESS_TOKEN".to_string(),
                "launch".to_string(),
                "---END---".to_string(),
            ]
        );
    }

    // XXX: key features are not tested
}
