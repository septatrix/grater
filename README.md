# Grater

> Nothing but a cheese grater for grades!

## Installation instructions

First you have to obtain a patched tabulate-java:

```shell
git clone git@github.com:tabulapdf/tabula-java.git
cd tabula-java/
# Fish shell syntax!
git apply (curl https://raw.githubusercontent.com/sp1ritCS/tabula-rs/master/0001-add-ffi-constructor-to-CommandLineApp.patch | psub)
mvn compile assembly:single
```
