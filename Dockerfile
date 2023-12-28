FROM rust:latest

RUN apt update && apt install git -y

RUN cargo install ibc-relayer-cli --bin hermes --locked