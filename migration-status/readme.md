# Install
```
apt install rustup
rustup default stable
```

```
apt install libssl-dev pkg-config build-essential fontconfig libfontconfig1-dev
```

# Running
Create `results/analysis` and `temp` folders.

```
nohup cargo run --release analysis > logs.txt 2>&1 &
```