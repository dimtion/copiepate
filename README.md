# Copiepate

[![Build](https://github.com/dimtion/copiepate/actions/workflows/build.yml/badge.svg)](https://github.com/dimtion/copiepate/actions/workflows/build.yml)
![Crates.io](https://img.shields.io/crates/v/copiepate)
![License](https://img.shields.io/github/license/dimtion/copiepate)

Copiepate is a small utility to remotely set the content of a clipboard.

I created this tool as I frequently use a remote tmux+vim setup and I often
need to copy a vim register to my local desktop.

## Usage

On your local desktop start the daemon in server mode and forward the port 2323
using ssh:

```bash
# Start copiepate server and listen on 127.0.0.1:2323:
copiepate --server

# In another shell, forward the server port to a remote machine:
ssh remote-machine -R 2323:localhost:2323
```

On the remote machine, copiepate sends the content of stdin to the local
machine clipboard:
```bash
# Set the clipboard content of the local machine:
echo -n "New clipboard content" | copiepate
```

## Setup and Installation

Using Rust Cargo:
```bash
# On GNU+Linux you'll need xorg-dev libraries.
# On other OSes (MacOS and Windows) this step is unecessary.

# Ubuntu/Debian:
sudo apt install xorg-dev libxcb-shape0-dev libxcb-xfixes0-dev

# Install copiepate (both server and client) using cargo:
cargo install copiepate


# IMPORTANT: generate a unique secret key that will be shared
# between on the client and the server using openssl:
openssl rand -base64 32

# Create a configuration file (both server and client) with a scret:
mkdir -p ~/.config/copiepate
cat  << EOF > ~/.config/copiepate/config.toml
secret = "<insert secret generated by openssl>"
EOF
```

## Vim integration

You can use copiepate to send the content of a vim register over the network:
```vim
" Using Plug as plugin manager:
Plug 'dimtion/copiepate', { 'rtp': 'vim' }
```

This plugin creates the following default bindings:
```vim
" Send the default register
noremap <leader>y :CopiePateReg<CR>

" In visual mode, send current selection
vnoremap <leader>y :CopiePate<CR>
```

## Configuration file

Copiepate supports having a configuration file to persist configuration.
Every setting can either be stored in the config file or by a command line argument.
Any setting set in the configuration file will be overriden by parameters passed
 to the command line.

 See full list of settings running  `copiepate --help`.

```toml
# Copiepate XDG configuration file:
# ~/.config/copiepate/config.toml

# Bind to a specific address
# Optional, default = 127.0.0.1
address = "192.168.0.2"

# Bind to a non default port
# Optional, default = 2323
port = "2325"

# Set a secret in base64 format
secret = "/f7NyvhS4k90gnstzXVPk/SpRl/Ex4EX9tyHRA2rT0w="

# [Server only]
# Specify a shell command to invoke whenever a paste event is received.
# Optional, default = ""
#
# Some examples:
# Show notification in MacOS:
# exec = "xargs -I % -0 -n 1 osascript -e \"display notification \\\"%\\\" with title \\\"Copiepate\\\"\""
#
# Show notification on GNU+Linux:
# exec = "xargs -I % -0 -n 1 notify-send \"%\""
#
# Log copy events to disk:
# exec = "cat >> copiepate_events.log"
#
# Ring terminal bell:
exec = "echo -en \"\007\""

# [Client only]
# Use copiepate as a passthrough. This allows to split an stdin between the send event and stdout.
# Optional, default = false
#
# Usage example:
# $ input_process | copiepate --tee > remove_copy_of_input_process.txt
tee = true
```

## Note on security

In its default configuration, copiepate listens only on the localhost address,
meaning that the port is not exposed to the local network.

WARNING: copiepate use encryption to ensure that attackers can't send paste event
or evedrop what messages are in transit over the network. However copiepate was
not audited. I recommend to only listen on a localhost port and only forward the port
using a secure protocol such as SSH, and not expose copiepate server to a local
 network or the internet.
