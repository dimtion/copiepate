# Copiepate

[![Build](https://github.com/dimtion/copiepate/actions/workflows/rust.yml/badge.svg)](https://github.com/dimtion/copiepate/actions/workflows/rust.yml)

Copiepate is a small utility to remotly set the content of a clipboard.

I created this tool as I use a lot a remote tmux+vim setup and I often need to
copy a vim register to my local desktop.

## Installation

Using rust package manager:
```bash
# On GNU+Linux you'll need xorg-dev libraries.
# On other OSes (MacOS and Windows) this step is unecessary.
sudo apt install xorg-dev libxcb-shape0-dev libxcb-xfixes0-dev

# Install copiepate
cargo install copiepate
```

Using rust package manager:
```bash
# To compile copiepate on Linux you'll need xorg-dev libraries:
sudo apt install xorg-dev libxcb-shape0-dev libxcb-xfixes0-dev
cargo install copiepate
```

## Usage

On your local desktop start the daemon in server mode and forward the port 2323
via ssh:

```bash
# Start copiepate in server mode
copiepate --server

# In another shell, forward the server port to the remote machine
ssh remote-machine -R 2323:localhost:2323
```

On the remote machine:
```bash
# Set the clipboard content of the local machine:
echo -n "New clipboard content" | copiepate
```

## Notes
In its default configuration, copiepate listens only on the localhost address,
meaning that the port is not exposed to the local network.

WARNING: There is no authentication and encryption over the network other than the
ssh tunnel. Meaning that any local service can in theory write to the clipboard
knowing the port.

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
