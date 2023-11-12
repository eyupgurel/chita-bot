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

## Error codes:
### Kucoin:
- 300003: Insufficient Balance
- 300012: Market bound exceeded
- 100004: Order can not be cancelled as its executed or does not exist