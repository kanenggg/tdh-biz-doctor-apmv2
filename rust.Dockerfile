# ---- Chef Stage ----
FROM rust:1.93 AS chef
RUN cargo install cargo-chef
WORKDIR /app

# ---- Planner Stage ----
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---- Builder Stage ----
FROM chef AS builder

ARG MODULE_NAME
ENV MODULE_NAME=${MODULE_NAME}

# Copy recipe and build dependencies (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=ssh \
  mkdir -p -m 0700 ~/.ssh && \
  ssh-keyscan bitbucket.org >> ~/.ssh/known_hosts && \
  cargo chef cook --release --recipe-path recipe.json

# Copy source and build application
COPY . .
RUN --mount=type=ssh \
  mkdir -p -m 0700 ~/.ssh && \
  ssh-keyscan bitbucket.org >> ~/.ssh/known_hosts && \
  cargo build -p ${MODULE_NAME} --release && \
  cp /app/target/release/$MODULE_NAME /app/target/release/application && \
  mkdir -p /app/${MODULE_NAME}/config && \
  cp -r /app/$MODULE_NAME/config/ /app/target/

# ---- Final Stage ----
FROM gcr.io/distroless/cc-debian13 AS runtime
ARG MODULE_NAME
ENV MODULE_NAME=${MODULE_NAME}
WORKDIR /app

COPY --from=builder /app/target/release/application .
COPY --from=builder /app/target/config ./config/

CMD ["./application"]

