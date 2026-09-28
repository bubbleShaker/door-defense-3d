# 営業おことわり！

しつこい不動産訪問営業を玄関先で撃退する 3D ゲームです。Rust + [Bevy](https://bevyengine.org/) で書いて WebAssembly にビルドしているので、ブラウザでそのまま遊べます。

▶ **プレイ:** https://bubbleshaker.github.io/door-defense-3d/

## 遊び方

休日の午後、営業マンが次々と門から玄関に向かってきます。玄関に居座られてインターホンを連打されると **忍耐ゲージ** が減り、0 になると根負けして契約させられてしまいます。全 5 ウェーブをしのげばクリアです。

| 操作 | 内容 |
| --- | --- |
| マウス移動 / タップ | 狙う |
| 左クリック（長押しで連射） / タップ | 投げる |
| `1` `2` `3` / `Q` / ホイール / 画面下のボタン | 武器の切替 |
| `Esc` / `P` | 一時停止 |

**武器**

- **塩** — 連射できるお清め。1 粒は弱いが手数で勝負
- **水バケツ** — 着弾点のまわりにまとめて効き、門のほうへ押し返す
- **お断りステッカー** — 強力で 3 人まで貫通。クールダウン長め

**営業マン**

- 新人営業 — 遅くて打たれ弱い
- ベテラン営業 — ジグザグに近づき、名刺を投げてくる（撃ち落とせる）
- 営業チーム — 2〜3 人組でやってくる
- 支店長 — 最終ウェーブのボス。パンフレットを 3 枚ずつ投げてくる

## ビルド

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk      # 未導入なら
trunk serve              # http://127.0.0.1:8080 で起動
trunk build --release    # dist/ に出力
```

`main` に push すると GitHub Actions（`.github/workflows/pages.yml`）がビルドして GitHub Pages にデプロイします。

## 素材

- 3D モデル・効果音は外部ファイルを使わず、プリミティブ形状とコード上の波形合成で作っています
- フォント: [Noto Sans CJK JP](https://github.com/notofonts/noto-cjk)（SIL Open Font License 1.1）。ゲーム内で使う文字だけにサブセット化したものを `assets/fonts/` に同梱しています。文言を変えたら `tools/make-font.sh` で作り直してください
