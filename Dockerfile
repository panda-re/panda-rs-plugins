FROM pandare/pandadev:latest

ENV PANDA_PATH="/panda/build"
ENV PATH="/root/.cargo/bin:${PATH}"

RUN apt-get -qq update && DEBIAN_FRONTEND=noninteractive apt-get install -y curl
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
RUN mkdir /panda/panda/rs-plugins
COPY .  /panda/panda/rs-plugins/
RUN bash /panda/panda/rs-plugins/install_plugins.sh

