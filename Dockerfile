    FROM node:25.1-bullseye
    # Install dependencies and GitHub Copilot CLI
    RUN apt-get update && apt-get install -y curl git
    RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg \
        && chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg \
        && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | tee /etc/apt/sources.list.d/github-cli.list > /dev/null \
        && apt-get update \
        && apt-get install -y gh
    # Install GitHub Copilot CLI (replace with the correct installation method if needed)
    # This might involve using 'npm install -g @github/copilot-cli' or similar
    # For demonstration, we'll assume gh cli is sufficient for basic interaction
    RUN npm install -g @github/copilot
    RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    

