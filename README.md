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

Run this command:

```
docker run \
-d -p 10666:10666 \
-v $PWD/ferrisbot.toml:/etc/ferrisbot.toml \
stuckoverflow/ferrisbot:latest
```

Check the logs with `docker logs <container-id>` (the container-id is the output
of the command above, or you can find it with `docker ps`): you'll find the url
to visit to proceed with the authentication flow.

### Persist the authentication token

By default, the authentication token will be lost every time the docker is
restarted, therefore the authentication flow will be required at every restart.
If you want to persist the token so it's available on restarts you can use
[Docker volumes](https://docs.docker.com/storage/volumes/), and set the
`token_filepath` in the `ferrisbot.toml` appropriately, e.g.

ferrisbot.toml:

```toml
[twitch]
...
token_filepath = '/token/auth_token.json'
```

Create the volume on your machine (only needed once):

```
docker volume create auth_token
```

Add a `-v auth_token:/token` option to the `docker run` command used to run the
bot:

```
docker run \
-d -p 10666:10666 \
-v auth_token:/token \
-v $PWD/ferrisbot.toml:/etc/ferrisbot.toml \
stuckoverflow/ferrisbot:latest
```
