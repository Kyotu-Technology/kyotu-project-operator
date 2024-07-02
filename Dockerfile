FROM lukemathwalker/cargo-chef:latest-rust-1.79.0 AS chef
WORKDIR /usr/src/kyotu-project-operator

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
COPY --from=planner /usr/src/kyotu-project-operator/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin kyotu-project-operator


FROM debian:bookworm-slim
ARG APP=/home/rust_app

RUN apt-get update && apt-get install -y wget tzdata ca-certificates libssl-dev openssl openssh-client && rm -rf /var/lib/apt/lists/*

ENV TZ=Etc/UTC \
    APP_USER=rust_app

RUN groupadd $APP_USER && useradd -g $APP_USER $APP_USER  && mkdir -p ${APP}
RUN chsh -s /usr/bin/nonlogin root

COPY ./templates ${APP}/templates
COPY --from=builder /usr/src/kyotu-project-operator/target/release/kyotu-project-operator ${APP}/kyotu-project-operator

RUN chown -R $APP_USER:$APP_USER ${APP}


USER $APP_USER

#add github to known hosts
RUN mkdir -p /home/rust_app/.ssh && ssh-keyscan github.com >> /home/rust_app/.ssh/known_hosts
RUN chmod 700 /home/rust_app/.ssh && chmod 600 /home/rust_app/.ssh/known_hosts
RUN chown -R $APP_USER:$APP_USER /home/rust_app/.ssh

WORKDIR ${APP}

EXPOSE 8080
CMD ["./kyotu-project-operator"]
