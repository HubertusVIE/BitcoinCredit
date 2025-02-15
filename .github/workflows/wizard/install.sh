#!/bin/sh

# Check if Homebrew is already installed
if command -v brew >/dev/null 2>&1; then
    echo "Homebrew is already installed."
else
    # Install Homebrew
    export SUDO_ASKPASS="$(pwd)/askpass.sh"
    yes "" | INTERACTIVE=1 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

    # Add Homebrew to PATH
    echo >> ~/.zprofile
    echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> ~/.zprofile
    eval "$(/opt/homebrew/bin/brew shellenv)"

    echo "Homebrew installation complete!"
fi

# Check if OpenSSL is installed through Brew
if brew list | grep -q openssl; then
    echo "✅ OpenSSL is installed: $(brew info openssl | head -n 1)"
else
    echo "⚠️ OpenSSL is not installed. Installing..."
    brew install openssl -q

    # Get OpenSSL path
    OPENSSL_PATH=$(brew --prefix openssl)

    # Verify OpenSSL installation
    if [ -x "$OPENSSL_PATH/bin/openssl" ]; then
        echo "✅ OpenSSL installed successfully: $($OPENSSL_PATH/bin/openssl version)"
    else
        echo "❌ OpenSSL installation failed."
        exit 1
    fi
fi