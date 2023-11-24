# chita-bot


## How to use
- The project requires rust and cargo >= `v1.71.1`
- Update `.env` file before running the bot

    | Command                    | Description           |
    | -------------------------- | --------------------- |
    | `cargo build`              | Build the project     |
    | `cargo run`                | Start the engine      |
    | `cargo test`               | Run tests             |
    | `cargo test -- --nocapture`| Run tests with output |
    | `cargo watch --exec test`  | Watch tests           |


#### Running as a system service
- Copy ``./chita-bot.service`` to path `/etc/systemd/system/chita-bot.service` 
- Update  target, env and working directory paths in `.service` file if needed. 
- `sudo systemctl daemon-reload`
- `sudo systemctl enable chita-bot.service`
- `sudo systemctl start chita-bot` - start bot
- `sudo systemctl status chita-bot` - check status of the bot
- The logs are produced at `/logs/chita-bot.log` Please use `tail -f /logs/chita-bot.log` to view the logs

## Error codes:
### Kucoin:
- 300003: Insufficient Balance
- 300012: Market bound exceeded
- 100004: Order can not be cancelled as its executed or does not exist
- 429000: Rate limit, too many requests