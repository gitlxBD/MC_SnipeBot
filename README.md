MC_SnipeBot
===========

A bot designed to efficiently snipe Minecraft usernames.

------------------------------------------
🛠️ Step 1: Install Rust
------------------------------------------

1. Go to https://rustup.rs/
2. Download rustup-init.exe
3. Run the executable as administrator
4. In the command prompt that opens, type:
   2
   Y
   1

------------------------------------------
📦 Step 2: Install Python dependencies
------------------------------------------

1. Open a new command prompt
2. Navigate to the folder where requirements.txt is located
3. Run the following command:

   pip install -r requirements.txt

------------------------------------------
⚙️ Step 3: Compile the Rust code
------------------------------------------

1. Open a new command prompt
2. Navigate to the mc_snipbot_v4 folder:

   cd path\to\mc_snipbot_v4

3. Compile the project in release mode:

   cargo build --release

------------------------------------------
🔑 Step 4: Get your Minecraft token
------------------------------------------

1. Open a new command prompt
2. Navigate to the folder where mc_token.py is located:

   cd path\to\folder_with_mc_token_py

3. Run the script:

   python3 mc_token.py

4. Follow the on-screen instructions to retrieve your token

------------------------------------------
🚀 Step 5: Snipe the desired username
------------------------------------------

1. Launch the sniping tool:

   mc_snipbot_v4\target\release\snipebot.exe

   (You can also double-click snipebot.exe from the file explorer)

2. Fill in the required information in the GUI.

⚠️ WARNING: The token you enter in the GUI must NOT contain any spaces
