FROM rust:1-buster
USER root

RUN apt-get update && apt-get -y install protobuf-compiler libclang-dev clang binutils-dev libunwind-dev

RUN mkdir -p /fuzztest

ADD . /fuzztest/

WORKDIR /fuzztest/template-fuzzer

CMD ["/bin/bash", "/fuzztest/template-fuzzer/run-fuzzer.sh"]