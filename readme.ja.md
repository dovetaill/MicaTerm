<div align="center">
  <p>
    <a href="readme.md">English</a> |
    <a href="readme.zh-CN.md">简体中文</a> |
    <strong>日本語</strong>
  </p>

  <img src="assets/icons/mica-term-app.svg" width="112" alt="Mica Term ロゴ">

  <h1>Mica Term</h1>

  <p><strong>ターミナル、リモートサーバー、その周辺ツールを一つにまとめるデスクトップワークスペース。</strong></p>

  <p>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-2024-000000?style=flat-square&logo=rust" alt="Rust 2024"></a>
    <a href="https://slint.dev/"><img src="https://img.shields.io/badge/UI-Slint%201.15-2379F4?style=flat-square" alt="Slint 1.15"></a>
    <img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-4F5D75?style=flat-square" alt="Windows、Linux、macOS">
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-E6B450?style=flat-square" alt="MIT License"></a>
  </p>
</div>

Mica Term は、モダンなターミナル、SSH 接続、SFTP ファイル管理、再利用可能なスニペット、認証情報、暗号化された設定同期を一つのネイティブデスクトップアプリに統合します。ローカルシェルとリモートマシンを日常的に行き来する人のために、静かで高速な作業環境を提供します。

> [!IMPORTANT]
> Mica Term は現在活発に開発されています。現在のバージョンは `0.1.0` であり、最初の安定版までに動作、保存形式、インターフェースが変更される可能性があります。

## 主な機能

| 分野 | Mica Term が提供するもの |
| --- | --- |
| ターミナル | WezTerm ベースのターミナルコア、タブセッション、Unicode シェーピング、カラー絵文字、テーマ、選択、URL オープン、コマンドブロック、Windows ネイティブ描画パス。 |
| SSH | 保存済み接続、パスワードと SSH キー認証、known-host 検証、接続進行表示、SOCKS5/HTTP プロキシ、SSH ジャンプホスト。 |
| SFTP | ターミナルの作業ディレクトリを認識するファイルブラウザー、履歴ナビゲーション、ディレクトリ追従、転送キュー、競合処理、再開可能な転送、独立ワークスペース。 |
| ワークスペース | クイック起動、セッションタブ、再接続と複製、コマンドパレット、複数行貼り付け確認、統合転送センター。 |
| アセット | SSH 接続、スニペット、スニペットパッケージ、ID、SSH キーを検索可能なツリーで管理し、組み込みデータベースへ保存。 |
| セキュア同期 | 暗号化 Vault スナップショット、マージと復旧、競合受信箱、Git リポジトリ、GitHub Gist、Gitee Gist、S3 互換ストレージ。GitLab Snippet コネクターは開発中です。 |

## Mica Term を選ぶ理由

- **リモート作業を一つの画面に集約。** ターミナル、リモートファイル、接続設定、スニペット、認証情報を同じ流れで扱えます。
- **ネイティブデスクトップの性能。** Rust と Slint で構築し、ブラウザーランタイムではなくプラットフォームに応じた描画経路を使用します。
- **堅牢な文字表示。** Unicode 分割、シェーピング、フォントフォールバック、グリッド記号、カラー絵文字を扱います。
- **セキュリティを意識した保存。** シークレットは OS のキーリングと連携し、同期 Vault は Argon2 と ChaCha20-Poly1305 で保護されます。
- **必要に応じてポータブル。** Windows パッケージでは `.mica-term-portable` マーカーにより、データとログを実行ファイルの横に保存できます。

## 技術スタック

| レイヤー | 技術 |
| --- | --- |
| 言語 | Rust 2024 edition |
| デスクトップ UI | Slint 1.15、Winit、software/Skia レンダラー |
| ターミナル | WezTerm terminal/surface forks、Termwiz |
| テキストと描画 | RustyBuzz、Swash、FontDB、Windows DirectWrite |
| 非同期ランタイム | Tokio |
| リモート接続 | Russh、Russh SFTP |
| ローカル保存 | Redb、Bincode、Serde |
| シークレットと暗号 | OS キーリング、Argon2、ChaCha20-Poly1305、Zeroize |
| 同期と通信 | Git2、Gix、Reqwest、OAuth 2.0、AWS SDK for S3 |
| 診断 | Tracing、ローテーションログ、クラッシュ記録 |

## アーキテクチャ概要

```text
Slint ビュー
    |
Shell と View Model の投影
    |
アプリケーションサービス
    |-- ターミナルコアとレンダラー
    |-- SSH セッションと接続ルーティング
    |-- SFTP ブラウザーと転送エンジン
    |-- アセット、キーリング、ローカル保存
    `-- 暗号化 Vault とリモート Provider
```

UI は宣言的に保ち、Rust が状態、検証、非同期処理、永続化、プラットフォーム統合を担当します。これにより、画面描画とターミナルやリモートセッションの動作を分離しています。

## はじめに

### 必要な環境

- 最新の安定版 Rust ツールチェーンと Cargo
- 対象プラットフォームのビルドツール
- Debian/Ubuntu では `pkg-config` と `libwayland-dev`

Linux のビルド依存関係とクロスコンパイルの詳細は [apt-packages.md](apt-packages.md) を参照してください。

### ソースから実行

```bash
git clone https://github.com/dovetaill/MicaTerm.git
cd MicaTerm
cargo run
```

### テスト

```bash
cargo test
```

### パッケージ作成

```bash
# Linux x64
./build-desktop.sh

# Windows x64 MSVC + Skia
./build-win-x64.sh

# Linux x64 と Windows GNU/software の一括ビルド
./build-release.sh
```

`build-desktop.sh` は `TARGET` 環境変数により Linux ARM64、macOS Intel/Apple Silicon、Windows GNU/MSVC にも対応します。完全な一覧は `./build-desktop.sh --help` で確認できます。

## プロジェクト構成

```text
src/app/        ターミナル、SSH、SFTP、Vault、永続化、プラットフォームサービス
src/shell/      ワークスペース状態、投影、ナビゲーション、UI 制御
src/theme/      ランタイムテーマ定義
ui/             Slint ビュー、コンポーネント、Shell、デザイントークン
assets/         ブランド素材、アプリアイコン、同梱フォント
tests/          振る舞い、回帰、統合、UI コントラクトテスト
scripts/        開発、検証、アセットツール
```

## コントリビューション

Issue と Pull Request を歓迎します。変更範囲を明確にし、動作上のリスクに応じたテストを追加し、提出前に関連する Rust テストと Shell コントラクトテストを実行してください。

## ライセンス

Mica Term は [MIT License](LICENSE) のもとで提供されます。
