use crate::connector::converge::converge;

mod models;
mod r#type;

mod bluefin;
mod sockets;
mod connector;
mod env;

#[tokio::main]
async fn main() {
        converge().await;
}
