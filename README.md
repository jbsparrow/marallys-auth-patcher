# Marallys Authentication Patcher

# Installation

## Installation (Windows)
Video guide: https://youtu.be/LUqX4GtlUQU<br></br>
1. Install [Prism launcher](https://prismlauncher.org/).
2. Click "Add Instance"
3. Set version to 1.20.1
4. Scroll down, and select Fabric as the mod loader. Choose version 0.16.10
5. Click OK
<br></br>
6. Click 'Folder'
7. Create a new folder and call it 'minecraft'
8. Open the Marallys launcher
9. Launch SMP or RP, depending on which version you want your profile to work for
10. After it launches, navigate to `%appdata%\MarallysLauncher\clients\`, then navigate to the profile you chose (SMP or RP) within that folder.
11. Copy all files from that folder to the 'minecraft' folder you created earlier.
<br></br>
13. Download the patcher and injector from [Releases](https://github.com/jbsparrow/marallys-auth-patcher/releases/latest)
14. Navigate to the profile folder (Click 'Folder' in Prism)
15. Place the patcher and injector into this folder
<br></br>
16. Hold shift and right click on the patcher, click 'Copy as path'
17. In Prism, click 'Edit' on the modpack, then click 'Settings'
18. Within the settings, click 'Custom commands'
19. In 'Wrapper Commands', paste the path you copied, your username, your password for hub.marallys.com, and then the authentication server URL (http://95.165.98.176:5000/api/v1/integrations/authlib/minecraft).<br></br>
    It should look something like this: <img width="916" alt="prismlauncher Console_window_for_1 20 1_-_Prism_Launcher_9 2 28-03-25 1832x263 f71953df-150c-4fe6-9e" src="https://github.com/user-attachments/assets/f91f0c9b-1be7-4663-80ec-1251c3582343" />
<br></br>
21. The setup is complete. You can now launch your game through prism with no logging to sentry or restrictions on what mods you can use.

## Installation (MacOS)
Coming Soon

## Installation (Linux)
Coming Soon

---

Credits: https://github.com/CatMe0w/mmcai_rs for the original project to implement the auth injection into Prism
