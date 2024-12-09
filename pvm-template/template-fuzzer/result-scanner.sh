#!/bin/zsh

data=0;
for file in output/template-fuzzer/crashes/**/*(.); do
	cargo ziggy run -i $file 2>output/$data.err 1>output/$data.out
	data=$((data+1))
done
