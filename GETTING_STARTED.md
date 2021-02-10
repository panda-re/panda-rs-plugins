## Installing the Rust Toolchain

Per the [official instructions](https://www.rust-lang.org/learn/get-started), install `rustup` like so:

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

If it's been a while since you've done this, you can upgrade the compiler/toolchain by running `rustup update`.

## Linking Rust plugins against your system's PANDA

If are using PyPANDA via the pip package ([see `pandare` here](https://pypi.org/project/pandare/)), you can skip this step.

If you have built PANDA from source or are inside of the official Docker container, you'll need to set the `PANDA_PATH` environment variable to the location of PANDA build directory.

If you cloned the PANDA repo in your home directory before building and your `panda-system-x86_64` executable is located at `/home/user_name/panda/build/x86_64-softmmu/panda-system-x86_64`, then `/home/user_name/panda/build/` is your build directory. You'd set `PANDA_PATH` like so:

```
export PANDA_PATH="/home/user_name/panda/build/"
```

You can make setting persistant by adding the above line to your `~/.bashrc`, updating `/etc/environment`, or whatever configuration your shell uses.

You're now ready to build Rust plugins! From a Rust plugin's directory, running `cargo build` will build the plugin. The `.so` file can be found under `<plugin_directory>/target/debug/lib<panda_name>.so`.

## Example: `panda-il-trace`

At present, this plugin requires an additional step to grab a specific version of `capstone`:

```
cd panda-il-trace
./setup.sh
```

You can use the [example PyPANDA script](./take_test_recording.py) to take a recording, or provide your own. Once you have a recording, you can build and run the plugin on it with:

```
cd panda-il-trace
./run.sh <name_of_process_to_trace> <name_of_recording>
```