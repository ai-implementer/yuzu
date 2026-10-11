---
title: 社内で公開する
order: 9
description: 公開範囲の注意（dist に出るもの・止め方）と、GitHub Pages 以外の Web サーバへの置き方
---

# 社内で公開する

yuzu が出力する `dist/` は静的ファイルだけです。置いた場所の公開範囲が、そのまま
サイトの公開範囲になります。社内だけに見せたいときは、アクセスの制限（社内ネットワーク
だけに限る・認証を掛ける）を Web サーバの側で行ってください。yuzu 自身はページ単位の
認証を持ちません。

> [!WARNING]
> GitHub Pages は、リポジトリを非公開にしてもサイトはインターネットに公開されます
> （GitHub Enterprise Cloud の非公開の Pages を除く）。`yuzu new` が作る
> `deploy.yml` は GitHub Pages へ公開するものなので、社内の設計書では使う前に
> 公開範囲を確かめてください。

## dist に出るもの

原稿から次のものが出力されます。公開したくないものがあれば、右の列の方法で止めます。

| 出力 | 中身 | 止め方 |
| --- | --- | --- |
| ページの HTML | 本文。HTML コメントもページのソースに残る | — |
| 原稿の `.md` | 原文そのまま（frontmatter・HTML コメント込み） | `llms.page_md = false`（全体）/ frontmatter `pageMd: false`（ページ） |
| `llms.txt` / `llms-full.txt` | ページの索引と全文（frontmatter は入らず、HTML コメントは入る） | `llms.enabled = false` / `llms.full = false` / frontmatter `llms: false` |
| `_search/` | 検索インデックス（本文のテキスト。HTML コメントは入らない） | `search.enabled = false` |
| `sitemap.xml` | 全ページの URL（`base_url` がフル URL のときだけ） | `base_url` をパスだけにする |
| `public/` の中身 | そのままコピー | 置かない |

右の列の設定（`page_md`・`pageMd`・`llms.*`・`search.enabled`・`base_url`）で止めると、
前回の build で出したものは次の build で `dist/` から消えます（`output.clean = false` の
サイトや、`--force` の後でも）。消したページや `public/` から外したファイルは、
`output.clean = false` のサイトで `--force` した後などに残ることがあります。社内のサイトでは
`output.clean` を既定の `true` のままにしてください。

`draft: true` のページは出力されません（`--drafts` を付けたときだけ）。

公開したくないメモは原稿に書かないのが確実です。とくに HTML コメント（`<!-- … -->`）は
ブラウザの表示には出ませんが、ページのソースと `llms-full.txt` に残ります
（[LLM 連携](llms.md#原稿の-md-を配信しない)）。

## Web サーバに置く

`yuzu build` の後、`dist/` の中身を Web サーバの公開ディレクトリへコピーします。
ページは `guide/` のようなディレクトリ形式の URL で、実体は `guide/index.html` です。
Web サーバ側では次の 2 つを設定してください。

- ディレクトリへの要求に `index.html` を返す
- 存在しないパスに `404.html` を返す（yuzu が生成します）

検索の wasm（`_search/search_bg.wasm`）は `application/wasm` で配ってください。
違う型で返すと、ブラウザのコンソールに警告が出て読み込みが少し遅くなります
（検索は動きます）。

### nginx

```nginx
server {
    listen 443 ssl;
    server_name docs.example.internal;
    root /srv/docs/dist;

    # 社内ネットワークだけに限る例（認証を掛けるなら auth_basic など）
    allow 10.0.0.0/8;
    deny all;

    location / {
        try_files $uri $uri/ =404;
    }
    error_page 404 /404.html;
}
```

サブパス（`https://docs.example.internal/design/`）で配るときは、`base_url = "/design/"`
でビルドし、`dist/` の中身を公開ディレクトリの `design/` に置きます。

### GitLab Pages

GitLab Pages は CI が成果物として渡した `public/` を配ります。yuzu の `public/`
（静的ファイルの置き場）は build のときに `dist/` へコピー済みなので、build の後で
入れ替えます。

```yaml
# .gitlab-ci.yml
pages:
  image: rust:latest
  script:
    - cargo install --locked --git https://github.com/ai-implementer/yuzu --tag vX.Y.Z yuzu-cli
    - yuzu build --base-url "$CI_PAGES_URL/"
    - rm -rf public && mv dist public
  artifacts:
    paths:
      - public
  rules:
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
```

`vX.Y.Z` は使う yuzu の版に置き換えてください。公開範囲はプロジェクトの
Settings の Pages の設定（アクセス制御）で決まります。

### Amazon S3

```bash
yuzu build
aws s3 sync dist/ s3://my-docs-bucket/ --delete
```

S3 の静的ウェブサイトホスティングを使うなら、インデックスドキュメントを `index.html`、
エラードキュメントを `404.html` にします。CloudFront から S3 の REST エンドポイントを
読む構成では、`/guide/` を `/guide/index.html` へ書き換える仕組み（CloudFront
Functions など）が別に要ります。アクセスの制限は、バケットポリシーでの接続元の制限や、
前段の CloudFront・社内の認証プロキシで行ってください。

## `base_url` の決め方

- ドメインの直下で配るなら指定しません（既定の `/`）
- サブパスで配るなら、そのパスを書きます（`base_url = "/design/"`）
- フル URL（`https://…/`）を書くと、`sitemap.xml` と共有カード（canonical・OGP）が
  出ます。社内向けで検索エンジンに載せないなら、パスだけで足ります
- 社内の DNS 名と IP アドレスなど、複数の名前で開かれるサイトはパスだけにしておくと、
  どの名前で開いてもリンクが壊れません

検索エンジンに載せたくないときは、`public/robots.txt` に `Disallow: /` を書いて
置きます（アクセスの制限の代わりにはなりません）。

## dev / preview は手元での確認用

`yuzu dev` と `yuzu preview` は手元で確かめるためのサーバです。社内への公開には使わず、
`dist/` を上のような Web サーバに置いてください。どちらも `localhost`・IP アドレス・
設定の `dev.allowed_hosts` 以外の Host からの要求を断ります（DNS リバインディングで
外部のサイトから読まれないための検査。[設定リファレンス](../reference/config.md#build--dev)）。
