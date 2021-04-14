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

## Run the bot via Docker

You can run the bot via Docker using a prebuilt image. You will still need to
prepare a `ferrisbot.toml` as described in the previous section. You have to
pass the `ferrisbot.toml` to the docker image using the `-v` option. You also
need to forward the port `10666` which is needed for the authentication flow.

The preferred way to run the mode is via `docker-compose`. For your convenience
please refer to the [`docker-compose.yml`](docker-compose.yml) file.
