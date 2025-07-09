# MC_SnipeBot
Step 1: -install rust by going on https://rustup.rs/
        -download rustup-init.exe
        -launch it in admininstrator
        -type 1 in the command prompt that opens
        
Step 2 : -open a new cmd prompt 
         -install the requirements by typing "pip install -r requirements.txt" (becareful to be in the right directory, same as the txt file)
         
Step 3 : compile the rust code in "mc_snipbot_v4" folder ;
         -open a new cmd prompt and go to the "mc_snipbot_v4" folder
         -type "cargo build --release" 
         
Step 4 : Get your MC token ;
         -open a new cmd prompt
         -go to the same folder as "mc_token.py" 
         -type "python3 mc_token.py"
         -follow the instructions 
         
Step 5 (final) : Snipe the username you want ;
                 -open mc_snipbot_v4 > target > release > snipebot.exe
                 -enter the infomations you need 
      WARNING: The token you enter in the GUI must NOT have any spaces 
