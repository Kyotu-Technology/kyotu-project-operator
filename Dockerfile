FROM rust:1.69.0 as builder
WORKDIR /usr/src/kyotu-project-operator
COPY . .
RUN cargo install --path .

FROM debian:bullseye-slim
ARG APP=/home/rust_app

RUN apt-get update & apt-get install -y wget extra-runtime-dependencies tzdata ca-certificates libssl-dev openssl & rm -rf /var/lib/apt/lists/*

ENV TZ=Etc/UTC \
    APP_USER=rust_app

RUN groupadd $APP_USER && useradd -g $APP_USER $APP_USER  && mkdir -p ${APP}
RUN chsh -s /usr/bin/nonlogin root

COPY --from=builder /usr/local/cargo/bin/kyotu-project-operator ${APP}/kyotu-project-operator

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER
WORKDIR ${APP}

EXPOSE 8080
CMD ["./kyotu-project-operator"]
