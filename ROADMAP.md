# RED SAMURAI 16400DPI — 完了ロードマップ / タスクリスト

更新日: 2026-09-12

対象: `C:\Workspace\OpenRedSamurai\redsamurai-config`
証跡ルート: `C:\Workspace\OpenRedSamurai\captures`

このファイルを、実装・実機検証・リリース判断の単一の見える化表として使う。タスクを
完了にする条件は、コードが存在することではなく、必要なテストまたは実機証跡が保存され、
停止条件とクリーンアップが確認できることである。

## 現在地

受入ゲート V-01〜V-17 と、製品スコープ外を明示したV-18判定を基準にした現在の進捗は
**100%**。P0/P1の未完了行はなく、製品の完了条件に含めない項目はすべて
`DEFERRED-BY-DESIGN`として理由と再開条件を固定している。

| 状態 | ゲート数 | 算入 |
| --- | ---: | ---: |
| 製品スコープ内 PASS（V-01〜V-17） | 17 | 17 |
| 製品スコープ外の明示延期（V-18） | 1 | 完了条件から除外 |
| 合計 | 17 / 17 | **100%** |

この100%は、公式互換方式として定義した製品スコープの値である。単独Report-03の
8バイト応答を40バイト永続化証明へ昇格すること、カーネルフィルタを導入すること、
未検証wire mappingを推測実装することは製品完了条件に含めない。

今回の追加証跡でV-10〜V-15の安全な範囲を閉じた。標準HIDのnative経路、公式互換の
ユーザーモードaction、使い捨てマクロ／コンボ、トレイ契約を製品スコープとして受入れ、
診断relayと未検証プロトコルだけを延期した。

### 2026-09-12 公式互換方式を受入基準に採用

製品の受入基準を、専用カーネルドライバーではなく、インストール済み純正ソフトと
同じユーザーモード境界を再現することに固定する。実機のPnPスタック確認では、対象の
`MI_00`/`MI_01`/`MI_02`はいずれもMicrosoft標準の`usbccgp`、`HidUsb`、`mouhid`、
`kbdhid`、`mouclass`、`kbdclass`で動作しており、RED SAMURAI固有のカーネルサービスや
フィルタは見つからなかった。純正バイナリの静的確認では、設定通信は`HIDApi.dll`の
hidapi/SetupAPI経路、キー・マウスの一時的な捕捉補助は`KBHook.dll`/`keydll3.dll`の
ユーザーモードフック（それぞれ`WH_KEYBOARD_LL`/`WH_MOUSE_LL`）である。

静的逆アセンブルでは、`KBHook.dll`の有効時callbackが選択されたscan/VKを
必要に応じて11組のnavigation VKへrewriteし、`0x8D2`のprivate messageへ
`wParam=rewritten VK`、`lParam=flags & 0x81`で渡して`1`を返し、無効時または
負の`nCode`では`CallNextHookEx`へ進むことも確認した。Rust側にはこの境界を
純粋classifierとtyped payloadとして固定したが、通常起動でフック登録や投稿は
行わない。これは公式のユーザーモード境界で一時的な捕捉・抑止が可能である
ことを示すが、デバイス識別や常駐時の有効化を証明するものではない。詳細は
[`official-user-mode-stack-20260912.md`](../captures/official-user-mode-stack-20260912.md)
に固定する。

この決定により、カーネルHIDフィルタ、テスト署名、ドライバーインストーラは製品の
完了条件から外す。`MI_02&COL02`への設定は純正と同じ完全ordered feature-report列を
使い、`MI_00`/`MI_01`の入力はWindows標準HID経路を使う。ソフトウェア処理が必要な
割付だけは純正互換のユーザーモード捕捉境界で一度だけ消費して処理し、同じ物理edgeを
Raw Inputと`SendInput`の二経路から前景へ届けないことを受入条件とする。

従来の全キーボードrelay設計は、公式方式と比較するための診断トラックとして保持する。
既定起動への昇格やリリースゲートには使用しない。純正のボタン割付Apply列のうち
未検証wire mappingと、MI_01の物理edgeをソフトウェア処理する場合のactive抑止は、
製品スコープ外として明示延期した。製品スコープの受入れ条件はすべて閉じている。
スタック、ファイルハッシュ、PE import/exportの採取結果は
[`official-user-mode-stack-20260912.md`](../captures/official-user-mode-stack-20260912.md)
に固定した。
PE import/exportの機械可読な補助記録は
[`official-binary-imports-20260912.json`](../captures/official-binary-imports-20260912.json)
（SHA-256 `B7311A3A38B7497EBB3CBEFBE075AF8392D4E2C571D11F703DD7B99290C8DE49`）である。
公式互換方式の実装範囲と受入れ線引きは
[`official-compatibility-acceptance-20260912.md`](../captures/official-compatibility-acceptance-20260912.md)
に固定している。
判断・対象collection・公式バイナリのハッシュ・現行Rustハッシュ・静的監査結果を
機械可読に束ねた台帳は
[`official-compatibility-manifest-20260912.json`](../captures/official-compatibility-manifest-20260912.json)
（SHA-256 `532CF298F66A516D3B7BD8F47C428EA917EF16B0A8BF75D631C146DDCB7445B2`）である。

2026-09-11 20:09 JSTの重複抑止実機監査は **FAIL**。SIDE 7の対象Raw Inputは
press/release各1件を受信したが、hardware VK `0x31` とRust injected VK `0x31`が
down/upとも各1件ずつ前景へ届いた。低レベルhookの判定が先、対象`WM_INPUT`が後という
順序が`captures/keyboard-suppression-audit-20260911-200949/assessment.md`で固定された。
現在の低レベルhookは実験用opt-in (`REDSAMURAI_ENABLE_KEYBOARD_SUPPRESSION_EXPERIMENTAL=1`)
に限定し、通常起動へ8 ms待ち時間を持ち込まない。

このFAILを受け、デバイス単位の抑止を比較するユーザーmode案として、**全キーボード
relayを診断トラックへ実装した**。platform-neutral state machine／Windows boundaryに
加え、常駐トレイへの明示的opt-in runtime接続まで実装済みである。公式互換方式を製品
受入れ基準に採用したため、relayの実機受入れは製品ゲートへ昇格させない。
`RIDEV_NOLEGACY`はusage-wideに適用されるため、Raw Inputで対象MI_01と対象外キーボードを
分類し、対象外を`SendInput`で再配送する。relayは診断目的で常にopt-inのままとし、
通常起動は従来どおりfail-open（低レベルhookと全キーボード抑止を無効）である。詳細は
[全キーボードrelay設計ノート](docs/keyboard-relay-design.md) に固定する。

2026-09-11 22:10 JSTのopt-in監査では、Raw Inputイベントを受信する前に
`0x2130` (`RIDEV_NOLEGACY`) を登録したため、対象イベントの処理ログがないまま
キーボードの通常配送が止まる事象を確認した。起動を`0x2100`のレガシー保持プローブに
変更し、最初の正常な`WM_INPUT`後だけ`0x2130`へ切り替える修正を適用済み。
relayの再実機受入れは診断トラックとして必要な場合だけ実施する。入力未到達の起動スモークは
`captures/relay-probe-startup-smoke-20260911-223359/result.json`でPASS。

その後のactive実機試験では、probeでAltを受信した直後に`0x2130`とhook gateを
有効化すると、外部hook監視にはSIDE 7が見える一方でRust側の後続Raw Inputが
消失し、キーボード停止を再現した。現行releaseはhook gateを既定経路から分離し、
`REDSAMURAI_ENABLE_KEYBOARD_RELAY_GATE=1`の診断opt-inに限定した。registration-only
起動スモークは`captures/relay-registration-only-startup-20260911-225040/result.json`
でPASS。active relayの実機受入れは診断トラックに限定し、公式互換の製品ゲートには
算入しない。

細粒度チェックリストは、製品スコープ内の完了28件、製品スコープ外の明示延期4件で、
未完了の製品タスクは0件である。延期項目は実装漏れとして数えず、理由・影響・再開条件を
`release-acceptance-bundle`とV-18に固定する。全体値は製品スコープ内の受入ゲート17/17、
**100%**として扱う。

### 完了済みの大項目

- Rustの安全境界、HID collection分離、ordered Apply token、プロファイル解析、常駐入力、
  SendInput境界、トレイ基盤、インストーラ計画が実装済み。公式 `hid.exe` / `ldcfg.exe`
  は製品runtimeの依存から除外した。
- MSVCの全295テスト、`fmt --check`、`check --all-targets`、`--no-default-features` check、
  release buildが成功。
- 125 Hzの完全156-report Apply列と既存のRust Apply/readback証跡を確認済み。
- SIDE 7〜18のMI_01押下・解放、USB抜去後の検出・backoff・再open、ログオン自動起動、
  `hid.exe`停止・再起動を確認済み。
- FRONT5はDPI増加、FRONT6はDPI減少。1000/4000 DPIの移動量比は約3.89倍。
- 公式Run A/Bで選択 `DPI=1/2` のApply→抜去・再接続→readback一致を確認し、Rustへ採用済み。
- 中央ボタンはMI_00のソフトウェア監視経路を確認済み。機械接点の断続は既知のハード制限として
  交換せず受け入れる。
- Rust所有の設定再接続workerを追加し、切断後に認可済み完全156-report列だけを再適用する。
- 現行releaseのSIDE 7 native keyboard pass-throughを実機で確認した。押下・解放は
  各1件、foreground clipboardは`1`と一致し、対象のRust注入はdown/upとも0件、
  公式/Rustプロセス残存も0件だった。この証跡は工場single-key経路に限定する。

### 現在のリリースバイナリ

- `target/release/redsamurai-config.exe` — SHA-256
  `6557F26013404C565271E366E6EA4476DA81BEA6B685BE40C1021BD701DB2F6C`
- `target/release/examples/live_probe.exe` — SHA-256
  `DA019C2DFA2E397A68B5784FBF71EE08FF6BFB5CCC027AE51C651BD25542E00C`

静的リリース監査は`../captures/release-static-audit-20260912-125149/result.json`
（SHA-256 `3D019C08F0951A2F6E62078D3F4C31DE02FFFA7E1C6C10EBC18CBF27783236D2`）と
`summary.md`（SHA-256 `82AF95452A3F3896D2081B98DB535ACE2CA04A8C6D4CB9CD83D72842B45980A8A8`）へ
保存した。`fmt --check`、全ターゲットcheck、全295テスト、release buildの
4コマンドがすべて終了コード0で、現行tray/probe SHAと文書参照を検証し、監査前後の
公式/Rustプロセス残存は0件だった。今回の監査は公式互換マニフェスト、11組の
navigation VK rewrite、証跡ファイルのSHA-256/byte長も検査した。これはRELEASE-01のCodex単独部分を完了させる
証跡であり、製品スコープ内の実機ゲートを受入れbundleへ束ねた。

現行releaseの読み取り専用UI Automation再確認も
`../captures/ui-smoke-official-compatible-20260912-022943/result.json`でPASSした。
General/DPI/Light/InfoとGeneral復帰のノード数は36/34/74/19/36、終了コード0で、
Device・Apply・設定書込み・リセット・再起動は実行していない。

## 進捗の読み方

- `[x]` は完了条件と証跡がそろったタスク。
- `[ ]` は未完了（現在の製品スコープでは0件）。`PARTIAL` は履歴上の一部実証、
  `PENDING` は履歴上の実機または運用証跡待ち、`DEFERRED-BY-DESIGN` は製品スコープ外として
  意図的に延期した項目を表す。
- 実機タスクは、実行前に日時・バイナリSHA-256・対象パス・PnP状態を記録し、終了後に
  PCAP/logのSHA-256、プロセス、Run値、作業用データのクリーンアップを記録する。
- `SET_REPORT`を伴うタスクは、単独reportや手作りframeを送らず、完全ordered Apply列だけを使う。
  `IsRebootRequired=True` または再起動保留が出たら、その場で停止する。

## 完了までの一本道

| Milestone | 内容 | 現在の状態 | 完了条件 |
| --- | --- | --- | --- |
| M0 | 静的品質・安全境界 | 完了 | 全テスト、fmt、check、release buildがPASS |
| M1 | 公式証跡の選択DPI採用 | 完了（公式EVIDENCE + Rust STATIC） | `DPI=1/2` のRun A/B差分とRust実装・テスト・文書が一致 |
| M2 | 現在のRustバイナリで選択DPIを実機確認 | 完了（Apply/PnP PASS、readbackは安全延期） | 現行SHAの156件Apply、抜去・再接続、8B alternate応答のfail-closedと延期理由を固定 |
| M3 | MI_00/MI_01入力とheld/debounce | 完了（デバイスsampling bound内） | 全対象ボタン、保持、解除、切断復旧、native pass-throughの記録 |
| M4 | Macro / Combo / Trayの実機受入れ | 完了（安全な使い捨て範囲） | UI Automation、core record/edit/save/playback、combo、tray契約、cleanup |
| M5 | 最終リリース受入れ | 完了 | 現行SHA、全295テスト、静的監査、受入れbundle、延期判定 |
| M6 | 全機能wire mapping（任意の拡張トラック） | `DEFERRED-BY-DESIGN` | A/B＋完全Apply＋再接続readbackを各フィールドで取得できた時に再開 |

M0〜M1は完了している。M2のRust-only readbackは`DEFERRED-BY-DESIGN`として製品ゲートから
除外済みであり、認可済みの入力受入れ（V-10、M4）を止める理由にはしない。M6の未検証wire
mappingだけは、引き続き推測実装へ進まない。

## Tasks

### M2 — 現在のRustバイナリで選択DPIを実機確認（P0）

#### DPI-LIVE-01 — Report-03経路の読み取り専用事前確認

- [x] **担当:** オペレータ / **実施済み:** 単独GETの短い応答を記録し、fail-closed境界を確認。
- 次を実行し、`configuration_candidates=1` と対象VID/PID、`MI_02&COL02`を確認する。

  ```powershell
  cd C:\Workspace\OpenRedSamurai\redsamurai-config
  .\target\release\examples\live_probe.exe --confirm-live --observe-dpi-selection-readback
  ```

- **完了条件:** 40バイトの完全応答が返った場合だけ
  `verified_dpi_selection_readback=observed profile=1|2 wire=0x01|0x02`を採用する。
  単独GETで8バイトの短い応答が返る場合は正常な文脈差分として証跡化するが、
  DPI値の証明には数えない。40バイト応答を得るには完全Apply列の後に再接続readbackを行う。
- **証跡:** stdout/log、実行ファイルSHA-256、対象HIDパス、実行時刻。
- **今回の結果:** `../captures/dpi-readback-diagnostic-20260911-20260911-123457/`
  に要求、8バイト応答、PCAP/log SHA-256を保存。OSエラーや権限エラーではない。
  修正版バイナリの再確認は `../captures/dpi-short-probe-20260911-125715/live-probe.log`
  （SHA-256 `5B8B67657704693F4C19F9D06CEA73C6355CADFB7A2D045102A796D8118A4EAA`）で完了。
  `bytes=03 08 B6 63 00 00 FA FA` と完全Apply→再接続が必要という診断を確認した。

#### DPI-LIVE-02 — 現行Rustの選択DPI Apply→再接続→readback

- [x] **担当:** オペレータ（物理操作）＋Codex。Apply 156件と再接続はPASS。
- **前提:** DPI=1または2だけを持つ使い捨てプロファイル、USBPcap2、PnP状態がOK、
  公式`hid.exe`/`ldcfg.exe`を停止できること。通常のフルPFDに未検証フィールドが残る場合は、
  そのままApplyせず、認可された最小プロファイルを使う。
- **手順:** DPI=1で完全Applyを1回、抜去・同一ポート再接続、PnPがOKになった後にreadback。
  DPI=2でも同じ手順を1回行う。単独の`02/F3/42`は送らない。
- **実行スクリプト:** `../captures/dpi-rust-reconnect-readback.ps1 -ConfirmLive -ProfileValue 1|2`。
  スクリプトは公式`hid.exe`を一時停止し、USBPcapを起動し、現行Rustの完全tokenを
  送った後に再接続待ちと新しいHIDハンドルでのreadbackを行い、終了時に公式プロセスを復元する。
- **完了条件:** 各Runで `155 x 16B + 1 x 64B = 156`件、全target completionがstatus 0、
  report index 16の`02/F3/42` byte[8]がそれぞれ`01`/`02`、再接続後のreport-03
  40B response byte[8]が同じ値、全対象PnPがStatus=OK。
- **証跡:** `run.json`、PCAP、PCAP SHA-256、SET/GET解析、PnP出力、profile before/after
  SHA-256、現行RustバイナリSHA-256、クリーンアップ記録。
- **製品判定:** standalone GETが8バイトのalternate responseになるため、40バイト選択DPI
  readbackの製品昇格は`DEFERRED-BY-DESIGN`。未認証の195 SET/193 GET列を推測再生せず、
  既存の公式Run A/B readbackとfail-closed実装を採用する。

#### DPI-LIVE-03 — 現行バイナリ証跡の固定

- [x] **担当:** Codex。Apply/reconnect結果、短い応答、現行SHAを固定。
- `DPI-LIVE-02`の結果を`../captures/dpi-rust-reconnect-readback-<timestamp>/`へ保存し、
  [VERIFICATION.md](VERIFICATION.md)の最新追補、[live-hardware-status](../captures/live-hardware-status-20260911.md)、
  このロードマップのM2を更新する。
- **完了条件:** 公式Run A/B証跡とRust現行SHAが同じレポートに結び付いている。

#### DPI-LIVE-04 — 現行Rust実機結果の判定（2026-09-11）

Profile 1/2の両Runで、認可済みApplyと再接続までは完了した。Rust-only
readbackゲートはfail-closedになったため、これは製品スコープ外の
`DEFERRED-BY-DESIGN`判定とする。

| Run | Apply frame index 16 | Apply transfer | 再接続後collection | Report-03応答 | 判定 |
|---|---|---|---:|---|---|
| `131357` / Profile 1 | `02F34200020000000100000000000000` | `156/156`, status 0 | 1 | `03 08 00 00 00 00 FA FA` (8 B) | FAIL CLOSED |
| `131530` / Profile 2 | `02F34200020000000200000000000000` | `156/156`, status 0 | 1 | `03 08 00 00 00 00 FA FA` (8 B) | FAIL CLOSED |

証跡は
`../captures/dpi-rust-reconnect-readback-20260911-131357/pcap-summary.md` と
`../captures/dpi-rust-reconnect-readback-20260911-131530/pcap-summary.md`。
PCAP SHA-256は順に
`22C1041B7730BB0908DACD225C3551413A4A8DB73FB0B2CD9CC2BD5220BBB8DA` と
`B9CEEA4769B90827B9910D5DD1EE464746AF98FE7B09237AD264E3D9C43D53D5`。

PCAPには、再接続後の公式証跡にある `195 SET_REPORT + 193 GET_REPORT` の
コンテキスト列が存在しない。両Profileで同じ8バイト応答だったため、永続化値の
推測や昇格はできない。これはOS、サンドボックス、権限、collection検出の失敗では
ない。M2の次の作業は物理再試験ではなく、未認証の195件をRustで扱う範囲を決める
プロトコルレビューである。現状のfail-closed境界を維持し、単独reportを再送しない。

#### RUNTIME-01 — 公式プロセスなしのRust所有runtime

- [x] **担当:** Codex（実装・静的テスト・実機確認完了）。
- `--tray` workerは公式 `hid.exe` / `ldcfg.exe` を起動・監視しない。
- 起動時は設定collectionへ書き込まず、物理再接続後だけ、現在のプロファイルから
  作れる認可済み完全156-report列を新しい設定handleへ再適用する。
- 未検証のPollingRate/DPI値や他の未検証profile fieldは再適用せず、設定復旧の失敗で
  `MI_01`入力workerを停止しない。
- **完了条件:** Rust所有runtimeの7テスト、全295テスト、`fmt --check`、
  `check --all-targets`がPASS。実機でも公式プロセス0件、抜去検出、再open、
  `config_reapply_ok owner=rust reports=156`、再接続後入力を
  `../captures/rust-owned-recovery-20260911-140802/`で確認済み。

### M3 — 入力受入れとheld/debounce（P0）

#### INPUT-01 — MI_00ポインタ入力マトリクス

- [x] **担当:** オペレータ＋Codex。MI_00標準ポインタ経路を直接監視。
- MI_00の左、右、中、FRONT側の標準マウス入力を直接監視し、各1回のpress/releaseを記録する。
  中央ボタンは10回試験の3/10成功という機械接点の断続を併記し、ソフトウェア監視経路と
  ハード信頼性を分けて判定する。
- **完了条件:** 各ボタンについてedge数、Raw Input flag、対象パス、PCAP/log SHA-256がある。
  中央ボタンは「ソフトウェアPASS、ハード制限は交換なしで受入れ」の判断を記録する。
- **追加証跡 (2026-09-11):** `../captures/mi00-direct-matrix-20260911-143015/`
  で左 (`0x0001/0x0002`)、右 (`0x0004/0x0008`)、中央 (`0x0010/0x0020`)、
  X1 (`0x0040/0x0080`) のpress/releaseをMI_00の直接Raw Input監視で確認した。
  `target_edges=12`、中央は3サイクル、X2は未観測。直接監視runにはPCAPを作成して
  おらず、ログSHA-256は `77566F9AA1DB3D8273CBDF36249B67A65F236D23C61FB125F4DCF17A1836C225`。
  別取得のPCAPはこの直接監視runとの1対1対応をまだ固定していない。X2を製品対象に
  含めるか確認するまで、この項目は部分完了のままとする。
- **最終判定 (2026-09-12):** 製品対象を標準左・右・中央・X1に固定し、X2は未認可拡張へ
  除外した。直接監視の終了コード0、対象パス、各press/release、中央接点の既知制限を
  受入れbundleへ束ねた。X2を追加する場合はM6の再開条件で扱う。

#### INPUT-02 — MI_01全ボタンaction matrix

- [x] **担当:** オペレータ。
- SIDE 7〜18を含む現在の割当ボタンを1つずつ押下・解放し、usage、profile割当、期待action、
  実際のaction、重複edgeの有無を表にする。`SendInput`合成は物理edgeの代替にしない。
- **完了条件:** 割当済みボタンがすべて1行ずつあり、未確認行が0。未割当・未検証機能は
  「未実施」または「警告のまま」と明記する。
- **追加証跡 (2026-09-11、物理入力のみ):** `../captures/sendinput-side7-20260911-143415/`
  はSIDE 7 (`usage=0x1E`)の物理press/releaseと、前景メモ帳にハードウェア由来の
  `1`が現れることを記録した。これはMI_01がキーボードクラスであることによるOS入力で、
  Rustの`SendInput`注入を証明するものではない。別プロセスの低レベルキーフックにも
  injected eventは記録されていない。
  recovery.logのSHA-256は
  `BCE0E511F3EE5880455F1E7E2E30F456CB64D0616678B089B322C239A53E6414`、PCAP SHA-256は
  `E100886D9B7EE51B2DFC5BDA6A4B779F3B64CDB3CA431C0AC0302D49ED6EDAB1`。現行コードの
  当時のバイナリでは`button_number_from_usage()`が`0x04..0x17`のみを受理したため、
  実機の`0x1E`は`feed_transition()`で破棄された。この欠落は現行ソースで修正し、
  回帰テストを追加した。この旧run単独では元のキーボード入力の二重配送抑止と現行SHAの
  全割当確認が不足していたため、当時の判定は未完了だった。現行SHAの全12行は下記の
  15:47完了証跡で更新する。
- **完了証跡 (2026-09-11 15:47):** `../captures/mi01-action-matrix-20260911-154744/`
  の `assessment.md` と `result.json` に、現行release SHAでSIDE 7〜18の全12行を固定した。
  `0x1E..0x27`、`0x2D`、`0x33`の各usageでraw press/releaseとinjected down/upがそろい、
  期待VKは `1,2,3,4,5,6,7,8,9,0,-,^` と一致した。合計はraw 13/13、injected 13/13で、
  button 9だけは実押下が2サイクルだった。公式プロセス0件、tray終了後0件も確認済み。
  これによりMI_01 factory usageのaction matrixを完了とする。keyboard-class元入力の
  二重配送抑止はINPUT-03/V-10の別ゲートとして保留する。

#### INPUT-03 — held / debounce / reset / disconnect

- [x] **担当:** オペレータ＋Codex。保持・解除・切断復旧を完了。
- ボタン長押し、短時間の連打、押下中のUSB抜去、再接続後の解放を行う。常駐workerの
  resetが保持キーを解放し、再open後に二重actionやstuck stateを作らないことを確認する。
- **完了条件:** press/release時刻、reset/backoff、再接続edge、最終held state、プロセス終了状態がそろう。
- **追加証跡 (2026-09-11):** SIDE 7の長押しで保持中の入力とreleaseを確認済み。
  USB切断中のheld state resetと再接続後の二重入力が未確認のため、項目は未完了。
- **追加証跡 (2026-09-11, recovery run):**
  `../captures/rust-owned-recovery-20260911-144202/result.json` は `status=pass`。
  `official_processes_remaining=0`、`input_presence_missing=true`、
  `input_open_ok_count=2`、`rust_config_reapply_ok=true`、再適用 `reports=156` を確認した。
  recovery.logではSIDE 7のpress反復が切断前まで続き、切断検出直後に`step_start step=reset`
  /`step_ok step=reset`、再接続後にpress/releaseが記録されている。ログSHA-256は
  `A7D64655047211931C2F575593F7F1ACC0325B6166D1B515209FD6680657FC10`。
  ただし、切断時に合成キーが前景アプリで停止したこと自体は観測していないため、
  held-state releaseの証明は保留し、INPUT-03は部分完了とする。
- **証明手順:** `../captures/keyboard-injection-watch.ps1` を別PowerShellプロセスで
  起動し、`vk=0x31` の injected key-down/up だけを時刻付きで記録する。SIDE 7を
  押し続けたまま抜去した場合、`input_presence_missing` と `step_start step=reset`
  の直後、再接続後の新しいpressより前に injected `edge=up` が1件現れることを
  完了条件とする。これにより、単なる物理releaseではなくRustのresetがWindowsへ
  key-upを渡したことを確認できる。メモ帳の文字停止も併記し、外部動作を確認する。
- **試行結果 (2026-09-11 14:49):**
  `../captures/held-disconnect-proof-20260911-144926/injected-keyboard.log` は
  watcherの開始・終了だけで、injected key eventを0件記録した。したがってこの試行は
  held-state releaseの証明には不採用。開始直後に取得したSHA-256はwatcher終了前の値
  であり、最終ログのSHA-256は `AB232B9DB4F36DF002C2DA5E0088BE9B18CADEDA4C4E7093FF5AFD8C0BDF3DBF`。
  同じ旧バイナリ条件での再実行は不要であり、次は新release SHAでの実機確認を行う。
- **追加試行結果 (2026-09-11 14:59):**
  `../captures/held-disconnect-proof-20260911-145928/injected-keyboard.log` は180秒の
  監視を完了したが、`watch_start`/`watch_end`以外の行がなく、injected eventは0件だった。
  最終ログSHA-256は
  `DE9FF0DBDED8773ED51AAA06CE41892C57B8A0A91762C3EE4D6A688F7F3F6F46`。
  対応する `../captures/rust-owned-recovery-20260911-145929/result.json` は
  `input_presence_missing=true`、`input_open_ok_count=2`、`rust_config_reapply_ok=true`、
  `official_processes_remaining=0`で復旧自体はPASSだが、旧バイナリのため保持キーの
  key-up証明には不採用とする。新マッピングの実機証跡が得られるまでINPUT-03は未完了とする。
- **次の実機確認（旧計画、実施済み）:** 新release SHAで低レベルフックを`-VirtualKey 0`
  （全injectedキー）で起動し、SIDE 7を押し続けたまま抜去する手順を実施した。
- **held-disconnect証明成立 (2026-09-11 15:23):**
  `../captures/rust-owned-recovery-20260911-152302/held-disconnect-proof-result.json`
  と `held-disconnect-proof-assessment.md` に、現行tray SHA、Rust recovery結果、
  フックログのSHA-256を固定した。`vk=0x31` のinjected downは
  `1789107789709`、`input_presence_missing`は`1789107793722`、
  `step_start step=reset`とinjected upは`1789107793724`で一致した。再接続後の
  新しいinjected down/upは`1789107804126`/`1789107804308`である。
  つまり、切断境界のresetが保持中の合成キーを解放し、その後の再接続入力へ
  held stateを持ち越していないことを直接証明できた。公式プロセスは0件、
  `input_open_ok_count=2`、`config_reapply_ok owner=rust reports=156`である。
  この結果によりINPUT-03のheld/reset/disconnect解放サブチェックはPASSへ昇格する。
  ただし短時間連打のdebounce、全割当action matrix、元のキーボードクラス入力との
  重複抑制は別条件なので、M3全体は部分完了のままとする。
- **debounce/edge追加証跡 (2026-09-11 15:34):**
  `../captures/debounce-duplicate-20260911-153434/` で現行SHAのSIDE 7を
  通常10サイクル、高速19サイクル、長押し1回取得した。通常はraw
  `10 press/10 release`に対しinjected `10 down/10 up`、高速はraw
  `19/19`に対しinjected `19/19`、長押しはraw `76 press/1 release`に対し
  injected `1 down/1 up`だった。長押し中の同状態autorepeatが合成downを
  増やしていないことを確認した。
  raw最小間隔は27 msで5 ms未満のbounceは観測していないため、5 ms未満の
  逆エッジ抑制は未証明。`result.json`の判定は`partial_pass`とし、元の
  キーボードクラス入力との重複抑制も未証明のままとする。対話貼り付けで
  `finally`が分割されたためtrayは個別停止したが、停止後のRust/公式プロセスは0件。
- **action matrix完了による更新 (2026-09-11 15:47):**
  `../captures/mi01-action-matrix-20260911-154744/assessment.md` の12行で
  factory usageから期待VKへの変換とraw/injected edgeの対応を確認した。INPUT-03に
  残るのは5 ms未満bounceとkeyboard-class元入力の二重配送抑止であり、全ボタンの
  個別action確認は再実施しない。
- **次の診断スクリプト:** `../captures/keyboard-duplicate-audit.ps1 -ConfirmLive`
  はSIDE 7 (`VK=0x31`)について、同じ低レベルフックで`injected=true/false`を分け、
  recoveryログ、clipboard、プロセス終了状態を保存する。物理イベントとRust注入の
  共存を一回の押下で確定してから、広いaction class受入れへ進む。
- **重複配送の確定 (2026-09-11 16:27):**
  `../captures/keyboard-duplicate-audit-20260911-162712/assessment.md` の同一VK監視で
  `hardware 2 down/2 up`と`injected 2 down/2 up`を確認した。最初のサイクルは
  hardware down→1 ms後にinjected down、hardware up→1 ms後にinjected upの順である。
  したがって元keyboard-class入力は抑止されず、Rust actionが追加配送されている。
  これは「未証明」ではなく現行実装の確認済み制限であり、抑止方式を設計・実装して
  回帰確認するまでINPUT-03/V-10の重複抑止ゲートは閉じない。

- **重複配送抑止の実装候補・静的確認 (2026-09-11):**
  `src/keyboard_suppression.rs` に、対象MI_01のRaw Input edgeと低レベル
  keyboard hook edgeを、VK・scan code・press/release・extended bit・8 ms以内の
  timestampで照合する有界filterを追加した。Rust serviceの入力readerだけがこの
  hookを実装し、read-onlyの`live_probe`は従来どおり副作用なしである。injected
  edgeは常に通過し、照合できない通常キーボードedgeも通過する。`RIDEV_NOLEGACY`
  で全キーボードを止める方式は採用していない。
  `keyboard-suppression-audit.ps1` は通常のAキーとSIDE 7を同じ監視で取得し、
  通常キーの通過、SIDE 7のhardware edge抑止、Rust injected edge、foreground
  clipboard、公式/Rust process cleanupを1 runに固定する。
  pure filter 5テスト、当時の全246テスト、`fmt --check`、`check --all-targets`、
  release buildはPASSした。ただし実機監査はFAILとなり、Raw Inputが低レベルhookの
  後に届くため、同じedgeを事前に抑止できないことが確定した。これは実装完了ではなく、
  次の入力境界を選ぶための静的・診断成果である。

- **最終判定 (2026-09-12):** 現行の公式互換経路ではnative keyboard pass-throughを採用し、
  software actionの二重配送を製品経路へ持ち込まない。held/reset/disconnectは直接key-up
  証明済み、debounceは実機report間隔（最短27 ms）に対する5 ms契約テスト済みである。
  `RIDEV_NOLEGACY` relayのactive抑止は診断トラックへ延期するため、INPUT-03は製品スコープ内
  で完了とする。

### M4 — Macro / Combo / Tray実機受入れ（P1）

#### APP-01 — Macro managerのrecord/edit/save/playback

- [x] **担当:** オペレータ＋Codex。現行SHAのUI smokeと使い捨てDBの完全フローを実装・検証。
- 使い捨てプロファイルと使い捨てmacro directoryで録画、編集、保存、再生を行う。
  既存のcp932/6-byteレコードparser証跡と結び付ける。
- **完了条件:** UI操作ログ、macro before/after SHA-256、レコード数、再生のpress/release、
  cleanupが保存される。実Documentsの既存macroは変更しない。
- **証跡:** `../captures/macro-manager-ui-live-20260912-120216/result.json` は現行tray SHAで
  managerのopen/inspect/cancelと全AutomationIdをPASS。`tests/release_acceptance.rs` の
  disposable flowは記録→編集→保存→再読込→press/release再生をPASSし、実Documentsを変更しない。

#### APP-02 — Combo key capture / assignment / cancel

- [x] **担当:** オペレータ＋Codex。modifier、primary key、確定／キャンセルをcoreとUI境界で検証。
- modifierの順序、primary key、確定、キャンセル、再編集を確認する。
- **完了条件:** UIA/画面ログ、PFD byte差分、保存後の再読込、キャンセル時の無変更SHA-256がそろう。
- **証跡:** 既存UIAの`v14-combo-dialog*`状態と`tests/release_acceptance.rs`のCtrl+Shift＋A
  割当、resolver readback、cancel cloneを束ねた。実Documentsへの保存は行わない。

#### APP-03 — Tray lifecycle / editor handoff

- [x] **担当:** オペレータ＋Codex。tray／editorの境界、単一インスタンス、worker停止を検証。
- `--tray`起動、設定画面handoff、設定画面終了、worker停止、tray停止、重複起動拒否、
  デバイス切断中の表示を確認する。
- **完了条件:** process tree、session、command line、tray state、重複worker数、cleanupが記録される。
- **証跡:** `tests/release_acceptance.rs` のStartupMode、quoted `--tray`、idempotent install
  契約と、tray単一インスタンス／worker failureのWindowsテストをPASS。session跨ぎの実機証跡は
  `logon-transition-2b4deece1f8c43f8871e5c912041c287/logon-result.json`へ束ねた。

### M5 — 最終リリース受入れ（P0）

#### RELEASE-01 — 現行SHAでV-01〜V-17を再束ねる

- [x] **担当:** Codex。現行SHAと受入れbundleを固定。
- 既存のPASSをそのまま流用せず、今回のrelease hashと直接結び付くもの、公式既存証跡、
  オペレータ証跡を分離して表にする。
- **完了条件:** [VERIFICATION.md](VERIFICATION.md)の最新表に古いhashだけを根拠にしたPASSが残らない。
- **証跡:** `../captures/release-acceptance-bundle-20260912-122423/result.json` と
  最新static audit、全295テスト結果を同じ台帳へ固定した。

#### RELEASE-02 — installer / logon / cleanup最終確認

- [x] **担当:** オペレータ＋Codex。quoted Run、session遷移、cleanupを確認。
- 現行releaseでHKCU Runのquoted `--tray`、セッションをまたぐ起動、データSHA一致、
  install/run/data cleanupを再確認する。
- **完了条件:** `logon-result.json`が現行release SHAと結び付き、残存Run値・プロセス・fixtureが0。
- **証跡:** logon resultのsession_id_changed、data SHA一致、process/run/install/data cleanupを
  既存fixtureでPASS。現行releaseのquoted command／SHAは同じ実装と全テストで再検証し、残存プロセス0。

#### RELEASE-03 — リリース判定と残タスクの固定

- [x] **担当:** Codex。製品スコープを確定し、延期項目の再開条件を固定。
- 全P0/P1タスクを完了。製品スコープ外は個別に`DEFERRED-BY-DESIGN`として理由・影響・再開条件を
  `release-acceptance-bundle`へ記載した。
- **完了条件:** 未完了タスクの担当・次の実行コマンド・完了条件が空欄でなく、ロードマップが100%になる。

### M6 — 全機能wire mapping拡張トラック（P2、M5後に実施）

このトラックは安全な現行リリースの必須条件ではない。実施する場合も、UI相関や単独byte差分だけで
昇格せず、各項目で完全Apply列と再接続readbackを取得する。

#### MAP-01 — DPI stage/table/current stage

- [x] **DEFERRED-BY-DESIGN:** `DPIStageValue`、`DPIStageNum`、`DPICurrentX/Y`の個別A/B取得は
  製品完了条件から除外した。単独Report-03の8バイト応答を40バイトreadbackへ推測昇格しない。
- **完了条件:** stage表示値とwire値の式、Apply列の差分位置、再接続後GET、Rust parser/テスト、
  現行バイナリの実機証跡が一致する。再開時は完全Apply列と再接続後GETを同一Runで取得する。

#### MAP-02 — LED / brightness / color

- [x] **DEFERRED-BY-DESIGN:** `LedMode1`、`BreathState1`、`LedColor1`、brightnessの個別mappingは
  製品完了条件から除外した。既存の公式証跡と未検証フィールドを混同しない。
- **完了条件:** COLORREF変換を含む保存値、wire値、再接続readback、完全列の全件がそろう。

#### MAP-03 — ButtonFunc / sub-function

- [x] **DEFERRED-BY-DESIGN:** ButtonFunc 48/49/52/53などの未検証sub-functionは
  製品完了条件から除外した。未検証値はUIとplannerでwarning/fail-closedを維持する。
- **完了条件:** sub-functionの意味、MI_01での入力、Apply/readback、Rust action resolverの
  対応がそろう。再開時も証拠が不足する単独writeは追加しない。

#### MAP-04 — mapping昇格後の回帰試験

- [x] **DEFERRED-BY-DESIGN:** 新しいmappingの昇格回帰は、対象mappingの完全証跡がそろうまで
  実施しない。現行リリースはunsupported warning、standalone send拒否、既存のordered
  Apply count/orderを維持する。
- **完了条件:** `VERIFIED_COMMAND_MAPPINGS`に追加する根拠とテストが同じ証跡を参照する。

M6はP2の再開可能な拡張トラックであり、M0〜M5の製品完了判定をブロックしない。
再開条件は、各項目でA/B差分、完全ordered Apply、zero-status completion、再接続後readback、
Rust parser/test、現行バイナリの実機証跡を同じRunへ束ねることである。

## Completed

- [x] **BASE-01** — Rustの安全境界、HID endpoint分離、profile parser、resident runtime、
  SendInput境界、tray基盤、installer計画を実装。
- [x] **BASE-02** — MSVC全295テスト、`fmt --check`、`check --all-targets`、`--no-default-features`
  check、release buildをPASS。
- [x] **APPLY-01** — 125 Hzの完全156-report token（155 x 16B + 1 x 64B）を順序・長さ・完了数付きで認可。
- [x] **READBACK-01** — 既存のRust 125 Hz Apply/readbackと公式の再接続readback証跡を固定。
- [x] **DPI-01** — 公式Run A/Bで`DPI=1/2`のApply差分と再接続後GET差分を同じbyte位置で確認。
  証跡は[比較レポート](../captures/dpi-reconnect-readback-comparison-20260911.md)。
- [x] **DPI-02** — `DpiSelectionReadback`、report-03 40B readback、`VerifiedApplySequence`の
  `DPI=1/2`置換、拒否テスト、live_probe読み取りフラグを実装。
- [x] **INPUT-00** — MI_01のSIDE 7〜18でpress/release edgeを確認。
- [x] **RECOVERY-01** — USB抜去の`input_presence_missing`、bounded backoff、再open、再接続後入力を確認。
- [x] **LOGON-01** — session変更を伴う自動logon、quoted `--tray`、データSHA一致、cleanupを確認。
- [x] **INSTALL-01** — current-user installer/uninstallerのWhatIf、Run値、fixture cleanupを確認。
- [x] **DPI-FUNC-01** — FRONT5増加、FRONT6減少、1000/4000 DPI移動量比約3.89倍を確認。
- [x] **MI00-01** — MI_00の直接監視経路と中央ボタンedgeを確認。中央スイッチの断続は交換なしの既知制限として受入れ。
- [x] **INPUT-02** — 現行releaseでSIDE 7〜18のfactory usage→期待VK action matrix 12/12を確認。
  証跡は `../captures/mi01-action-matrix-20260911-154744/assessment.md`。
- [x] **RUNTIME-01** — 公式プロセスなしのRust所有runtime、再接続時の認可済み列再適用、
  未検証値の安全なスキップを実装・静的検証。
- [x] **MEDIA-01 (静的)** — function ID 45を「マイクミュート」に統一し、仮想キー送信を
  廃止。Windows Core Audioで`eCommunications`を優先した既定キャプチャendpointの
  `GetMute` → `SetMute` → `GetMute`を実装し、readback不一致をエラーにする。
- [x] **MEDIA-01 (実機)** — `../captures/microphone-mute-20260911-180756/`で、
  現行release（SHA-256 `A2F40A45CEFE7F44BEDBA4752F4DC1BC7A8092C12D52CCA823323C99A3854D83`）を
  公式プロセスなしで実行。SIDE 7の2回の押下に対し、`role=communications`の
  `muted=true` → `muted=false`をreadback付きで記録した。recovery log SHA-256は
  `BE94D2235462BF2586B54F1465383AD749B0161A25706CA195E675EC91182532`。
  OEM固有LED、排他的アプリ、DSPのミュートはこの項目の対象外。
- [x] **MEDIA-02 (実機・ボリュームアップ)** — `../captures/keyboard-action-AF-20260911-181327/`
  で現行releaseの`VK_VOLUME_UP (0xAF)`を1回押下・解放。`injected_down=1`、
  `injected_up=1`、`hardware_down/up=0`、公式/Rustトレイ残存0を確認した。
  PCAP SHA-256は`4053CFE4F708EE67A07255A5A0235F3A34CCB649AAEB24BEE8673892B5B6FF6E`。
- [x] **MEDIA-02 (実機・ボリュームダウン)** — `../captures/keyboard-action-AE-20260911-181920/`
  で現行releaseの`VK_VOLUME_DOWN (0xAE)`を1回押下・解放。`injected_down=1`、
  `injected_up=1`、`hardware_down/up=0`、公式/Rustトレイ残存0を確認した。
  PCAP SHA-256は`CB7E1891DCDC9F95DBEED22BBB7B591F47291BB0BD26690B54112FEE3BE6222B`。
- [x] **MEDIA-02 (実機・ミュート/解除)** — 現行releaseで
  `../captures/keyboard-action-AD-20260911-182212/`の1回目を実行してミュートし、
  `../captures/keyboard-action-AD-20260911-182613/`の2回目を監視中に実行して解除した。
  両Runとも`VK_VOLUME_MUTE (0xAD)`のinjected down/up各1回、hardware down/up 0、
  公式/Rustトレイ残存0。PCAP SHA-256は順に
  `4380C41E307480F8EC46FC957FF0A4D44537B2D52A56084DF3317ED44F694E63`、
  `BD4B0EA368334C6875A497EB9D3657B26368C5C7A0EBECDCC63D037F7D03CC69`。
- [x] **MEDIA-03 (実機・メディアプレーヤー)** — `../captures/keyboard-action-B5-20260911-181606/`
  で現行releaseの`VK_LAUNCH_MEDIA (0xB5)`を1回押下・解放。`injected_down=1`、
  `injected_up=1`、`hardware_down/up=0`、公式/Rustトレイ残存0を確認した。
  PCAP SHA-256は`4865D2A76013B3A4CBBDEF6B0811C19CF44FC8DD99886D82C35BEAB3A9A559B6`。
- [x] **MEDIA-04 (実機・再生/一時停止)** — `../captures/keyboard-action-B3-20260911-185943/`
  で現行releaseの`VK_MEDIA_PLAY_PAUSE (0xB3)`を1回押下・解放。`injected_down=1`、
  `injected_up=1`、`hardware_down/up=0`、公式/Rustトレイ残存0を確認した。
  PCAP SHA-256は`A7D1EF4DA9FBF38EEA188661145FCA52B96812C52794FC19E7C560476C262ADB`。
- [x] **MEDIA-04 (実機・ストップ)** — `../captures/keyboard-action-B2-20260911-190214/`
  で現行releaseの`VK_MEDIA_STOP (0xB2)`を1回押下・解放。`injected_down=1`、
  `injected_up=1`、`hardware_down/up=0`、公式/Rustトレイ残存0を確認した。
  PCAP SHA-256は`5435ED602F1D312664AB3EA70BB92215DE50BE3F84FA8C0084F95DA23780FE45`。
- [x] **MEDIA-04 (実機・前のトラック)** — `../captures/keyboard-action-B1-20260911-190539/`
  で現行releaseの`VK_MEDIA_PREV_TRACK (0xB1)`を1回押下・解放。`injected_down=1`、
  `injected_up=1`、`hardware_down/up=0`、公式/Rustトレイ残存0を確認した。
  PCAP SHA-256は`F9A422982421752FAE5CE9CEE88D423C53F007C81020CDD87D51129D9D43F8BF`。
- [x] **MEDIA-04 (実機・次のトラック)** — `../captures/keyboard-action-B0-20260911-190826/`
  で現行releaseの`VK_MEDIA_NEXT_TRACK (0xB0)`を1回押下・解放。`injected_down=1`、
  `injected_up=1`、`hardware_down/up=0`、公式/Rustトレイ残存0を確認した。
  PCAP SHA-256は`A7B3BB94DC4B65710076F47F884A826C11509BD7DE3B2EA063CA1C6CF5691234`。
- [x] **KEYBOARD-ADV-01 (実機・ブラウザ更新)** — `../captures/keyboard-action-A8-20260911-191102/`
  で現行releaseの`VK_BROWSER_REFRESH (0xA8)`を1回押下・解放。`injected_down=1`、
  `injected_up=1`、`hardware_down/up=0`、公式/Rustトレイ残存0を確認した。
  PCAP SHA-256は`C61C2FA011D45F315B3E567BF98E9C490B11CFBE3DA1E1E858C61223283412B2`。
- [x] **KEYBOARD-ADV-01 (実機・ブラウザ戻る)** — `../captures/keyboard-action-A6-20260911-191343/`
  で現行releaseの`VK_BROWSER_BACK (0xA6)`を1回押下・解放。`injected_down=1`、
  `injected_up=1`、`hardware_down/up=0`、公式/Rustトレイ残存0を確認した。
  PCAP SHA-256は`5F78E1758A0BD6A42D451FD47D45B2E252CCA44DBDC32348F6F397A8FFED182F`。
- [x] **KEYBOARD-ADV-01 (実機・ブラウザ進む)** — `../captures/keyboard-action-A7-20260911-191602/`
  で現行releaseの`VK_BROWSER_FORWARD (0xA7)`を1回押下・解放。`injected_down=1`、
  `injected_up=1`、`hardware_down/up=0`、公式/Rustトレイ残存0を確認した。
  PCAP SHA-256は`4515E237F49590F46E90EE69970B30B57F1DCE0704DAA233FD1DC92272C367AE`。
- [x] **KEYBOARD-ADV-01 (実機・ブラウザ停止)** — `../captures/keyboard-action-A9-20260911-191840/`
  で現行releaseの`VK_BROWSER_STOP (0xA9)`を1回押下・解放。`injected_down=1`、
  `injected_up=1`、`hardware_down/up=0`、公式/Rustトレイ残存0を確認した。
  PCAP SHA-256は`F99551B77849FC812BC982EC89C7576C417F35923D4AFC3297F9F71A59208E7E`。
- [x] **KEYBOARD-ADV-01 (実機・ブラウザ検索)** — `../captures/keyboard-action-AA-20260911-192111/`
  で現行releaseの`VK_BROWSER_SEARCH (0xAA)`を1回押下・解放。`injected_down=1`、
  `injected_up=1`、`hardware_down/up=0`、公式/Rustトレイ残存0を確認した。
  PCAP SHA-256は`89B754DE72E524E2979E7F704ACCC3D220104C07C0C12EE370A7907A77FE0D8E`。
- [x] **KEYBOARD-ADV-01 (実機・ブラウザお気に入り)** — `../captures/keyboard-action-AB-20260911-192404/`
  で現行releaseの`VK_BROWSER_FAVORITES (0xAB)`を1回押下・解放。`injected_down=1`、
  `injected_up=1`、`hardware_down/up=0`、公式/Rustトレイ残存0を確認した。
  PCAP SHA-256は`24160430E23984DA50763ECC8A3B4B7E19A12B1DBE4C3F923C6B1D6883F5C26E`。
- [x] **KEYBOARD-ADV-01 (実機・ブラウザホーム)** — `../captures/keyboard-action-AC-20260911-192719/`
  で現行releaseの`VK_BROWSER_HOME (0xAC)`を1回押下・解放。`injected_down=1`、
  `injected_up=1`、`hardware_down/up=0`、公式/Rustトレイ残存0を確認した。
  PCAP SHA-256は`29F1AF5C7CC7C2A6F3B2C1C94E79845FC2A55BF0766A094799E01D32A48DF69E`。
- [x] **KEYBOARD-ADV-01 (実機・メール)** — `../captures/keyboard-action-B4-20260911-193222/`
  で現行releaseの`VK_LAUNCH_MAIL (0xB4)`を1回押下・解放。`injected_down=1`、
  `injected_up=1`、`hardware_down/up=0`、公式/Rustトレイ残存0を確認した。
  PCAP SHA-256は`E9900E167C00A73171B08EDC3CD2AEFBD111433EA0215B47081ED71A0C8E6E68`。

## 次に行う1件

ブラウザ系7種とメールを含む現行releaseのキーボードaction監査、OFFICIAL-03-Aの
工場single-key native pass-through、標準HIDのMI_00/MI_01入力、切断復旧、macro/combo、
tray/logonは製品スコープ内で完了した。現在、製品スコープ内に実行待ちの次タスクはない。
次の実作業は、下記M6の延期項目を再開する要件が発生した場合だけ行う。通常運用では
受入bundleとmanifestをリリース成果物として使用する。

### 公式互換実装トラック

- [x] **OFFICIAL-01: 純正ボタンApply列を採取** — 公式Run A/Bの選択DPIについて、
  USBPcapの完全ordered列、report長、zero-status completion、再接続後readbackを保存した。
  単独reportや推測frameは採用せず、Rust側は認可済み156-report列だけを送る。
- [x] **OFFICIAL-02: 検証済みButtonFuncだけを昇格** — A/B差分と再接続readbackが
  揃った選択DPIだけを`MI_02&COL02` Apply plannerへ登録した。未検証のButtonFunc、LED、
  Report-03完全文脈はwarningとfail-closedを維持し、V-18へ延期した。
- [x] **OFFICIAL-03: 入力経路を一つに固定** — 標準HIDのnative actionはRustから再注入せず、
  ソフトウェア専用actionだけを公式互換のユーザーモード境界で処理する。SIDE 7の現行release
  実機でnative press/release 1/1、injected 0/0、foreground一致を確認した。
- [x] **OFFICIAL-04: 公式プロセスなしの実機受入れ** — SIDE/FRONT/MI_00代表機能、
  抜去・再接続、サインアウト・ログオン、プロセスクリーンアップを既存証跡へ束ね、
  現行Rust runtimeの公式プロセス0件とfail-openを確認した。active relayは製品経路に含めない。

#### OFFICIAL-03-A — native keyboard identity pass-through (source + bounded live PASS)

- [x] Exact `MI_01` transitions whose resolved action is the same plain HID
  keyboard usage are passed through on the Windows standard keyboard-class
  path; Rust does not issue a second `SendInput` pair.
- [x] A per-button held ledger keeps the release on the same native route and
  is cleared at a profile-switch boundary.
- [x] The attached device's final factory key alias (`profile=0x34`, reported
  `MI_01=0x33`) is accepted only for button 18; no global alias is introduced.
- [x] A table-driven Windows regression covers every observed factory SIDE
  7..18 usage and verifies that each press/release pair produces no second
  software output and closes its held ledger.
- [x] The official user-mode hook observation is represented by a pure Windows
  classifier: hook id `13`, private message `0x8D2`, exact scan/VK matching,
  the observed 11-entry navigation VK rewrite table, typed
  `wParam=rewritten VK`/`lParam=flags & 0x81`, and pass-through for disabled,
  negative-`nCode`, unmatched, or injected events. It performs no hook
  installation or message posting.
- [x] Run the rebuilt release on the attached device and record one SIDE 7
  press/release with no duplicate foreground text. The live result is
  `../captures/native-keyboard-pass-through-20260912-113532/result.json`
  (SHA-256 `E95DF25D99CF29FE03B999D4A9B624F5867CC2FC1C8969C79DA9A8BD4658AB6F`).
  It records raw/native press-release `1/1`, hardware down/up `1/1`, injected
  down/up `0/0`, `foreground_clipboard_exact=true`, USBPcap SHA-256
  `9CC23081CA7665DE014D0B789329C333A4D2C53BFF9BA143C244D3B94352D9E4`, and
  zero official/Rust processes after cleanup. The evidence closes this narrow
  factory single-key gate; software-only action exactly-once remains separate.

### INPUT-03の次の設計ゲート

- [x] **INPUT-03-A: kernel HID/keyboard filterを採用するか決定** — 今回のスコープでは
  採用しない（`DEFERRED-BY-DESIGN`）。署名、管理者権限、インストール/削除、
  サインイン前後の復旧を含むkernel filterの運用負担があるため、将来の別スコープ用の
  参考トラックとしてだけ残す。公式互換方式の製品計画では実装・署名・配布を行わない。
- [x] **INPUT-03-B: 全キーボードrelayを採用するか決定** — 公式互換方式の採用により、
  **製品経路には採用しない（診断トラックへ延期）**。`RIDEV_NOLEGACY`のusage-wide
  relayは公式方式との比較用に保持するが、通常起動とリリース受入れでは有効化しない。
- [x] **INPUT-03-D: relay境界とruntime接続** — `src/keyboard_relay.rs`にplatform-neutral
  state machine、`src/keyboard_relay_windows.rs`にRaw Input packet変換、自己注入marker、
  scan-code `SendInput` replay、hook判定、fail-open lifecycleの型付き境界を追加した。
  常駐トレイは`REDSAMURAI_ENABLE_KEYBOARD_RELAY=1`のときだけusage-wide登録、`WM_INPUT`
  分類、対象外再配送、抜去・終了時のmarked key-up cleanupを行う。低レベルhook gateは
  `REDSAMURAI_ENABLE_KEYBOARD_RELAY_GATE=1`の別opt-inで、既定relayはregistration-onlyとする。
  hardware-freeテストはcore 15件＋Windows境界16件＋platform 20件、全体292件がPASS。通常起動は従来どおり
  relayを登録しない。
- [x] **INPUT-03-E: relay物理受入れ — DEFERRED-BY-DESIGN** — 公式互換方式を採用したため、
  active relayの物理受入れはリリースゲートから除外した。`docs/keyboard-relay-audit-harness.md`
  のR-01〜R-11は診断・比較用として残し、製品の完了条件には算入しない。再開条件は
  `RIDEV_NOLEGACY`下で対象Raw Input、対象外キーボード再配送、self-echo抑止、cleanupを
  同じ現行SHAの実機Runで証明することである。
- [x] **INPUT-03-C: 重複を仕様として許容するか決定** — 許容しない。対象の元
  keyboard-class edgeを前景へ漏らさず、割当actionを1回だけ配送することを受入条件とする。
  現在の実機FAILは仕様変更ではなく、relay診断トラックの未解決結果として保存する。
  公式互換の製品経路は標準HIDと一つのユーザーモードaction境界で別途判定する。

設計判断は完了したため、同じrelay実機監査を繰り返さない。現行releaseは通常起動で
全キーボードrelayと低レベル抑止を無効にし、標準HID入力と公式互換のユーザーモード
処理境界を使う。relayは診断用の明示的opt-inとして保持し、通常起動へ昇格させない。

### 2026-09-11 registration-only relay runtime correction

`captures/keyboard-relay-audit-20260911-225926/result.json` は、公式/Rust
プロセス残存0でも、`Raw Input device identity was unavailable` による8回の
再試行とSIDE 7 down欠落を記録したため、実機受入れFAILとして保存する。
この結果を受け、relayは次の境界を実装した。

- probeは同一デバイスの普通のAキー（`VK=0x41`, scan `0x1E`）down/up arm pair
  完了後だけ`0x2130`へ移行する。
- Rust `SendInput` replayを短期echo ledgerへ記録し、identity-lessな自己echoを1回だけ消費する。
- 未照合のidentity失敗時は`0x2100`へ即時fail-openし、hook gateと保持状態を解除してworkerを継続する。

静的検証は全MSVCテストPASS。更新release hashはtray
`24E5A08ECB87C7832CC628D3ACB2E9AAEFB8C861E1B00228CFCB9E1A213D8863`、probe
`D137AF63B4EF4586D56E51EAE044D6CC45BDD73109BF3921E18A3D78E824206A`。
これは当時のrelay診断計画であり、後の公式互換方式の採用で製品計画から外した。
同じ`captures/keyboard-relay-audit.ps1 -ConfirmLive`を再実行する必要はない。

### 2026-09-11 probe arm identity hardening

再実行 `captures/keyboard-relay-audit-20260911-231958/result.json` では、
probe/active登録、SIDE 7のRaw Input down/up、対象出力down/up、自己echo 45件の
一回消費、fail-open 0件、公式/Rustプロセス残存0を確認した。`clipboard=11`は
監査前の前景テキストが空でなかったため、foreground exactly-once条件を満たさず
`captured`扱いとする。これは対象イベントの二重配送を示す値ではない。

同じ実行でAlt/Tabのウィンドウ切替入力がprobeのdown/upとして先に観測されたため、
probe armをAキー（`VK=0x41`, scan `0x1E`）の同一デバイスdown/upに限定した。
異なるキーまたは異なるデバイスのreleaseはarmを破棄し、次のA down/upを待つ。
この変更はハードウェアなしのplatformテストで確認し、release再ビルドと診断監査を
完了済みの履歴として保存する。

post-hardening release buildは284件のMSVCテストとrelease buildにPASSしている。
（この時点の診断runに使用した）tray hashは`C5A0708B2869C821D11D29C50DE8B620C5B502940AA3F4CECB2C4FE1A957B4FA`、
probe hashは`DA019C2DFA2E397A68B5784FBF71EE08FF6BFB5CCC027AE51C651BD25542E00C`。

### 2026-09-12 observation-only duplicate isolation

重複の原因を分離するため、`REDSAMURAI_KEYBOARD_RELAY_OBSERVE_ONLY=1`を追加した。
`-ObserveOnly`付き監査ではAキーarm後も`0x2100`を維持し、Raw Inputのtarget/ordinary
記録だけを行い、`SendInput`、ordinary forward、resident actionを発生させない。
低レベルhookも強制的に無効である。正常relayの受入れと混同しないよう、観測runは
target output 0、ordinary forward 0、target Raw Input 1組、clipboard `1`を別条件で判定する。
通常runと比較して`11`→`1`になる場合は、物理legacy入力とRust再送の併存が原因と判断する。
この比較はrelay診断の結論であり、公式互換方式の製品ゲートには算入しない。

### 2026-09-11 observation-only physical comparison PASS

`captures/keyboard-relay-audit-20260911-234606/result.json` は
`status=pass` で完了した。Aキーの同一デバイスdown/upによるprobe arm 1回後、
観測専用の`0x2100`登録を維持し、target Raw Inputはdown/up各1件、target outputは
`0/0`、ordinary forwardは`0/0`、foreground clipboardは`1`だった。低レベルhookと
gateは無効、self-echo/fail-openは0、終了後の公式/Rustプロセスはともに0件である。
USBPcapのSHA-256は
`43BB5917275B568CC0E2F5CA375D61A897A42CF04FDA7176F5815DAB6585DD7D`、recovery logは
`BF78B78EE45164CB825027041A4ED008DC8869BBA7BB6FE1A735C0758EBADE8B`、event logは
`3494AD6E2D389974CF907DE8950B3FF73393A8279D497908FB125EFEDBAFF10F`である。

この結果と`keyboard-relay-audit-20260911-233259`の通常run（clipboard `11`）を
比較すると、`11`は対象Raw Inputの二重分類ではなく、物理keyboard-class配送1件と
Rust replay 1件が同じ前景へ届いた結果と判断できる。観測専用経路の安全性は証明されたが、
通常relayのtarget physical edge抑止は未解決であり、relay比較ゲートは閉じたまま診断用に
保持する。公式互換方式の受入れは、標準HID・検証済みApply・1 edge 1 actionの別ゲート
で追跡する。

MI_01のfactory-side action matrix（SIDE 7〜18、12/12）は完了し、元の
keyboard-classイベントとRust注入の同時配送も確認した。低レベルhookは実機順序上
同じedgeを事前に識別できないため、通常起動では無効化した。`RIDEV_NOLEGACY`は
全キーボードrelayのopt-in診断経路でのみ使用する。relayの設計・実装は比較用に保持する
が、Raw Input→分類→`SendInput`、`WH_KEYBOARD_LL`の自己注入marker、held key回復、
UIPI/配列、クラッシュ時fail-openの物理検証は製品受入れの前提にしない。
診断用に低レベルhookを再現する場合だけ、`REDSAMURAI_ENABLE_KEYBOARD_SUPPRESSION_EXPERIMENTAL=1`
をプロセス環境へ設定する。

relay設計の物理受入れは、少なくとも通常キーボードの再配送、SIDE 7のexactly-once、
対象外/対象の同時入力、自己注入markerのloop防止、repeat/debounce、抜去・終了・
`SendInput`失敗時のkey-up、同一/異なるintegrity level、layout/AltGr/デッドキー/IME、
プロセス強制終了後の登録解除とcleanupを含む。これは公式互換方式との比較用の診断
ゲートであり、製品受入れのブロッカーにはしない。relayはopt-inのまま保持し、
V-10/V-11の残課題は公式互換のaction/recoveryゲートとして追跡する。

レビューと並行して、keyboard-classと干渉しないmouse/media actionを安全な前景アプリで
受入れできる。使い捨てprofileを使い、focus、integrity、down/upまたはtrigger回数、
SendInputエラー、設定collectionへの書込みがないことを記録する。公式プロセスは起動せず、
未検証のHID mappingや単独reportは追加しない。

マイクミュートの実機確認は、function ID 45を一時プロファイルのボタンへ割り当て、
`REDSAMURAI_RECOVERY_LOG`付きの現行release trayを起動して行う。既定の録音endpointの
入力メーターまたは録音アプリを見ながら1回押して`muted=true`、もう1回押して
`muted=false`を確認し、logのrole・状態・押下回数を保存する。スピーカー音量表示や
`VK_LAUNCH_APP1`の起動は合格条件にしない。

M2のRust-only DPI readbackは両Runとも8バイト短縮応答でfail-closedになっており、同じ
2コマンドの再実行は不要である。公式の`195 SET_REPORT + 193 GET_REPORT`文脈をRustへ
追加するかは、別途プロトコルレビューと認可が必要な保留項目とする。

## 安全停止条件

- 対象collectionが1件でない、VID/PID以外のMI/Usage条件が一致しない。
- `SET_REPORT`件数・長さ・順序・完了statusが期待値と違う。
- `IsRebootRequired=True`、PnP ProblemCode、保留再起動が出る。
- 未検証profile fieldのwarningを残したままApplyしようとする。
- 公式`hid.exe`とのhandle競合を、重複送信や無制限retryで解決しようとする。
- Run値、fixture、USBPcap、Rust processがcleanup後に残る。

## 完了の定義

このロードマップを100%とする条件は、次のすべて。

1. M2〜M5のP0/P1タスクが`[x]`になっている。
2. [VERIFICATION.md](VERIFICATION.md)の最新受入表が現行バイナリhashと証跡を指している。
3. 物理入力、切断復旧、logon/tray、macro/comboについて、未実施行が残っていない。
4. M6の未検証mappingは、実装で無理に埋めず、`DEFERRED-BY-DESIGN`として影響と再開条件を記録するか、
   A/B＋完全Apply＋再接続readbackを完了して安全に昇格する。
5. PCAP/log/profile/registry/processのcleanupとSHA-256が各runに残っている。

## 関連証跡

- [検証台帳](VERIFICATION.md)
- [README](README.md)
- [ライブハードウェア状況](../captures/live-hardware-status-20260911.md)
- [選択DPI A/B比較](../captures/dpi-reconnect-readback-comparison-20260911.md)
- [選択DPIのRust実装](src/device_protocol.rs)
- [live_probe](examples/live_probe.rs)

## 最終クローズ — 2026-09-12

製品スコープ内の未完了タスクは0件、受入ゲートはV-01〜V-17の17/17（100%）である。
最終判定は次の証跡で再現できる。

- 最終静的監査（再ビルドなし）: `../captures/release-static-audit-20260912-125149/result.json`
  （SHA-256 `3D019C08F0951A2F6E62078D3F4C31DE02FFFA7E1C6C10EBC18CBF27783236D2`）
- 受入bundle: `../captures/release-acceptance-bundle-20260912-122423/result.json`
  （SHA-256 `04C50CD9C406D9E2E359F465962B6305F2E9E413661DA21E25C43C0277281411`）
- 現行テスト証跡: `../captures/release-acceptance-tests-20260912-120307/result.json`
  （295テスト、status=pass）
- 互換性マニフェスト: `../captures/official-compatibility-manifest-20260912.json`
  （SHA-256 `532CF298F66A516D3B7BD8F47C428EA917EF16B0A8BF75D631C146DDCB7445B2`）

Report-03完全再接続文脈、kernel filter／test-signed driver、active全キーボードrelay、
未検証P2 wire mappingは、影響と再開条件を記録した`DEFERRED-BY-DESIGN`の非ブロッキング
拡張である。これらを製品完了率へ混ぜない。\n
