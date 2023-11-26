#!/bin/bash

cargo build --release
cp target/release/amcheck bin/
