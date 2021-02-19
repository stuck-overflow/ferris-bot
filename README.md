# ferris-bot
Twitch bot for organising queues - developed live at twitch.tv/stuck_overflow

Use `cargo run -- --help` to see the available options.

## Twitch authentication flow

Register a new application at https://dev.twitch.tv/console/apps . When
configuring it, add as "OAuth Redirect URLs" parameter the address
`http://localhost:10666`. You should then obtain from the same page a Client ID
and a Client Secret. Add these parameters to your `ferrisbot.toml` configuration
file (see `sample.ferrisbot.toml` for an example).

On first run, the bot will print an URL you need to go to in order to complete
the authentication process.

## Discord authentication 

### 1. Register your bot 

To register a new application visit https://discord.com/developers/applications and create a new 

Select bot from the options, check the permissions for the bot to write to the chat and then copy

### 2. Obtain a channel ID

Right click the channel you wish to post to in Discord and select 'Copy ID'

### 3. Add credentials

Add the bot authorisation token and channel ID to your `ferrisbot.toml` file as described above. 

