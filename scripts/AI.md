
## Run gemini in container

### Build container with rust and gemini

```docker build -f ./Dockerfile-gemini . -t gemini```

### Run container

set GEMINI_API_KEY= in .env file then run

```docker run --rm -it --env-file=.env -v $PWD/android-adb-run:/workspace gemini```


