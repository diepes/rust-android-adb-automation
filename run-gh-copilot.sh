#!/usr/bin/env bash
echo "# Container lacks dependencies needed for Dioxus .."
docker build -t copilot-cli-container .
docker run -it -v ~/.config:/root/.config -v $PWD:/app copilot-cli-container bash
