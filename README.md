# Spelling — a SimpleEdit plugin

The words that the Hunspell dictionaries of the machine do not know,
underlined in [SimpleEdit](https://github.com/simple-edit-app/simpleedit): all
through prose (text, Markdown outside its code), and in the comments of
programs. **Ctrl+Alt+S** (the editor's shortcut, which can be changed) shows the suggestions for
the word at the cursor, adds it to the personal dictionary or ignores it.

The dictionaries are looked for in the plugin's data folder
(`dictionaries/`), `~/.local/share/hunspell`, `/usr/share/hunspell`,
`/usr/share/myspell`, Homebrew's and macOS's `~/Library/Spelling`: a `.dic`
with its `.aff`, as `apt install hunspell-fr` or `brew install hunspell`
leave them.

## Build

```sh
cargo test
cargo build --release && mkdir -p bin && cp target/release/spell bin/
```

## License

MIT
