## Template Fuzzer

This a fuzzer implementation for OpenZeppelin's runtime templates. Currently there is a single runtime (generic one) and a single fuzzer setup.
This code is highly experimental, if you notice any flaws consider creating an issue or a pull request.

### How to run the fuzzer

We have provided a docker packaging for the fuzzer, so that you can run it like this from the **repository root directory**

```bash
docker build -t fuzzer -f template-fuzzer/Dockerfile .
docker run --mount source=output,target=/fuzztest/template-fuzzer/output fuzzer
```