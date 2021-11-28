# Copiepate

[![Build](https://github.com/dimtion/copiepate/actions/workflows/build.yml/badge.svg)](https://github.com/dimtion/copiepate/actions/workflows/build.yml)
![Crates.io](https://img.shields.io/crates/v/copiepate)
![License](https://img.shields.io/github/license/dimtion/copiepate)

Copiepate is a small utility to remotely set the content of a clipboard.

I created this tool as I frequently use a remote tmux+vim setup and I often
need to copy a vim register to my local desktop.

## Installation

Using Rust Cargo:
```bash
# On GNU+Linux you'll need xorg-dev libraries.
# On other OSes (MacOS and Windows) this step is unecessary.
sudo apt install xorg-dev libxcb-shape0-dev libxcb-xfixes0-dev

# Install copiepate (both server and client):
cargo install copiepate
```

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

## Notes on security

In its default configuration, copiepate listens only on the localhost address,
meaning that the port is not exposed to the local network.

WARNING: There is no authentication and encryption over the network other than
the ssh tunnel. Meaning that any local process can write to the clipboard by
knowing copiepate server port.

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
