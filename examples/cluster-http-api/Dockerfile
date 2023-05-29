FROM ubuntu:20.04
RUN apt-get update
RUN apt-get install -y curl
RUN apt-get install -y git
RUN DEBIAN_FRONTEND=noninteractive apt-get install -y clang
RUN apt-get install -y libclang-dev
RUN apt-get install -y build-essential
RUN DEBIAN_FRONTEND=noninteractive apt-get install -y cmake
RUN apt-get install -y libcurl4-openssl-dev
RUN apt-get install -y libssl-dev
RUN apt-get install -y make
RUN apt-get install -y libssl-dev
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
RUN apt install pkg-config -y

ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR .
COPY node ./node
COPY examples/cluster-http-api .
RUN cargo install --path .
CMD ["cluster-http-api"]