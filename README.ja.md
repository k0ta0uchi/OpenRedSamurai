# redsamurai-config

英語版は [README.md](README.md) です。リリース手順と検証台帳は
[ROADMAP.md](ROADMAP.md) / [VERIFICATION.md](VERIFICATION.md) を参照してください。

現在の配布版は **v1.0.0** です。GitHub Releases の Windows x64 zip に含まれる
`setup.ps1` を、展開したフォルダーから PowerShell で実行すると、現在のユーザーだけに
インストールできます（管理者権限不要）。

RED SAMURAI 16400DPI Gaming Mouse (VID_04D9/PID_FC55) 用の設定ツール — 純正ソフトを
常駐させずに使えるRust + eguiによる再実装。

**Phase 1 (完了)**: UI 完全再現 + 純正プロファイル (.pfd) の読み書き完全互換
**Phase 2 foundation (実装済み)**: HID デバイス境界、観測済みレポート schema、
プロファイル適用 plan/dry-run、完全な125 Hz Apply列の認可ゲートと UI 配線

**Phase 3/4 (実装済み)**: `MI_01` の9バイト入力を読む常駐ランタイム、デバウンス付き
ソフトウェア割付・マクロ再生、Windows `SendInput` 境界、通知領域トレイ、`--tray` 自動起動、
HKCU用の型付きインストール計画とレビュー可能な PowerShell インストーラ

製品の受入基準は、専用カーネルドライバーではなく純正ソフトのユーザーモード方式を
再現することです。対象デバイスはWindows標準の`usbccgp`/`HidUsb`/`kbdhid`/`mouhid`
スタックで動作し、設定はhidapiのfeature report、ソフトウェア割付はユーザーモードの
入力境界で処理します。デバイスが既に生成している同一HIDキーボードusageはRustから
再注入せず標準経路を通し、ソフトウェア専用操作だけを一つのSendInput/Core Audio境界から
配送します。全キーボードrelayとカーネルフィルタは製品経路にせず、比較用の明示的opt-in
診断として保持します。製品スコープの要約と再現手順は
[`docs/evidence/release-acceptance.md`](docs/evidence/release-acceptance.md) にあります。
生のPCAP・ログ・互換性マニフェストは検証ワークステーション側のcapturesに保管し、
リポジトリへは取り込みません。

製品スコープ内の受入ゲートは **17/17、100%** です。現行release SHA、295テスト、
静的監査、実機入力・切断復旧、UI smoke、使い捨てmacro/combo、tray/logonを束ねた
最終判定は [docs/evidence/release-acceptance.md](docs/evidence/release-acceptance.md)
に要約しています。Report-03完全readback、kernel filter、
active全キーボードrelay、未検証P2 wire mappingは`DEFERRED-BY-DESIGN`であり、製品完了を
ブロックしません。

現行releaseのSIDE 7 native keyboard pass-through実機確認は
`../captures/native-keyboard-pass-through-20260912-113532/result.json` に保存しており、
raw/native・hardwareの押下/解放各1件、Rust注入0件、foreground clipboard完全一致、
公式/Rustプロセス残存0件を記録しています。これは工場single-key経路に限定した証跡です。

外部デバイス・Windows 実行・トレイ・SendInput・実レジストリのライブ受入れと未実施項目は
[VERIFICATION.md](VERIFICATION.md) に V-01〜V-18 として、前提・手順・停止条件・必要な証跡をまとめています。
全体の進捗、次に実行するタスク、完了条件は [ROADMAP.md](ROADMAP.md) に固定しています。
インストーラーの実HKCU・一時Documentsデータ境界を一度だけ検証する場合は、
`installer/live-integration.ps1` のGUID付き・明示トークン付きハーネスを使います。

Phase 2 の UI は起動時に HID を列挙・open・write しません。タイトルバーの
「デバイス」表示は未確認から始まり、クリックしたときだけ読み取り専用の軽量な
接続確認を行います。既存の「適用」ボタンだけが明示的な適用経路です。未接続時は
適用 plan を dry-run として保持し、未検証のフィールドは警告として表示して書き込みを
停止します。適用前のプロファイル保存に失敗した場合も、保存状態とデバイス状態が
食い違わないよう HID I/O を開始しません。

## ビルド / 実行 / テスト

MSVC 環境が必要です。同梱の `msvc_cargo.bat` (vcvars64 設定込み) を使います。
`cmd //c` は Git Bash 専用の書き方なので、シェルごとに次を使ってください:

**PowerShell:**

```powershell
.\msvc_cargo.bat build                        # ビルド
.\msvc_cargo.bat test                         # 全テスト
.\target\debug\redsamurai-config.exe          # 起動
```

現行releaseの静的受入れ（fmt、全ターゲットcheck、全テスト、release build、
バイナリSHA、文書・公式互換マニフェスト・rewrite表・証跡ハッシュ、プロセス残存）を
一括で記録する場合は、次を実行します。
デバイスI/O、再起動、リセット、既存プロセスの終了は行いません。

```powershell
& ..\captures\release-static-audit.ps1
```

常駐トレイモードはインストーラが登録する `--tray` で起動します。トレイの「設定を開く」は
通常の編集画面を別プロセスで開き、「終了」は常駐入力ループも停止します。Rustプロセスは
公式 `hid.exe` / `ldcfg.exe` を起動・監視しません。通常の起動では設定用 `MI_02&COL02` を
開かず、切断からの再接続時だけ、プロファイルから作れる認可済みの完全156-report列を
Rust自身が再適用します。未検証フィールドは送信せず、再適用できない場合も入力監視は
継続します。

```powershell
.\target\debug\redsamurai-config.exe --tray
```

ライブ境界を一度だけ確認する場合は、明示的な確認フラグ付きの
`examples/live_probe.rs` を使えます。既定では厳密なHID候補の列挙、任意の1回の
MI_01読み取り、またはゼロ距離マウス移動の`SendInput`だけを行い、設定用feature
reportは送信しません。証拠済みの125 Hz列を実機へ一度だけ送る場合だけ、追加の
`--apply-125hz --confirm-hid-apply`を指定します。これは`PollingRate=8`だけの
認可済みplanを作り、厳密な設定collectionが1件のときに156レポートを送ります。

実際のRaw Inputで物理的な押下・解放エッジを監視する場合は、必ず
`--confirm-live --read-resident --read-resident-for-ms <n>`を指定します。`<n>`は
ミリ秒で、最大30,000 msに制限されます。設定Applyや`SendInput`フラグとの併用は
拒否され、監視終了時には`resident_monitor=timeout`、Raw Inputスレッド切断時には
`resident_monitor=disconnect`が出力されます。監視は入力イベントを合成せず、受信した
押下・解放とHID usageをそのまま`resident_transition=press|release usage=0xNN`として
記録します。

この監視は `MI_01\\KBD` のキーボード形式コレクション（追加・マクロボタン）を
対象にします。通常の左・右・中クリックは `MI_00` の標準マウスコレクション
なので、この監視結果には現れません。

```powershell
.\msvc_cargo.bat run --example live_probe -- --confirm-live --read-resident
# 実際のRaw Inputを30秒だけ監視（上限も30,000 ms）:
.\msvc_cargo.bat run --release --example live_probe -- --confirm-live --read-resident --read-resident-for-ms 30000
# SendInputを確認する場合だけ追加:
.\msvc_cargo.bat run --example live_probe -- --confirm-live --sendinput-zero-move
# ゼロ距離がOSに拒否されたときの最小可視移動（1px）:
.\msvc_cargo.bat run --example live_probe -- --confirm-live --sendinput-one-pixel
# 設定HIDへ証拠済み125 Hz列を一度だけ送る場合だけ追加:
.\msvc_cargo.bat run --example live_probe -- --confirm-live --apply-125hz --confirm-hid-apply
# 選択DPIのReport-03経路を読み取り専用で確認（単独GETは値を証明しない）:
.\msvc_cargo.bat run --release --example live_probe -- --confirm-live --observe-dpi-selection-readback
# 現行RustのDPI=1/2を完全Applyし、物理再接続後に40バイトreadbackを確認:
..\captures\dpi-rust-reconnect-readback.ps1 -ConfirmLive -ProfileValue 1
```

単独のReport-03 GETでは、デバイスが8バイトの短い状態応答を返すことがある。
これはOS権限やサンドボックスの失敗ではなく、選択DPIを含まない別の応答である。
選択DPIの40バイト応答は、完全なApply列で`03/F3/42`を送った後の再接続readbackで
だけ検証する。短い応答を受けた場合、probeは応答バイトを表示してfail-closedで停止し、
値を`DPI=1/2`へ推測変換しない。

`dpi-rust-reconnect-readback.ps1`は過去の比較証跡用ハーネスで、公式プロセスを一時停止して
から復元します。製品の通常運用やRust所有の再接続復旧には公式プロセスは不要です。公式を
完全に停止した状態で入力復旧を確認する場合は、次を使います。

```powershell
& ..\captures\rust-owned-recovery.ps1 -ConfirmLive
```

再ビルドしたRustトレイで、工場設定のSIDE 7が標準HID経路だけで一度配送され、
同じキーをRustが再注入しないことを確認する場合は、次の専用監査を使います。
メモ帳を空にしてSIDE 7を短く1回押し、Ctrl+A→Ctrl+C後にEnterを押してください。

```powershell
& ..\captures\native-keyboard-pass-through-audit.ps1 -ConfirmLive
```

`rust-owned-recovery.ps1`は公式 `hid.exe` / `ldcfg.exe` が1件でも残っていると開始せず、Rustの
`input_presence_missing` → 再接続後の `input_open_ok` と、認可済み設定列の
`config_reapply_ok owner=rust`（対象外なら `config_reapply_skipped`）をログに固定します。
公式プロセス0件での実機確認は `captures/rust-owned-recovery-20260911-140802/` に保存済みです。

Windowsの`MI_01\\KBD`キーボードクラスは、昇格していても通常のHID
`ReadFile`を`ERROR_ACCESS_DENIED`で拒否するため、常駐入力は直接HIDハンドルを
読みません。厳密に列挙したパスをRaw Inputのデバイス名と照合し、専用のメッセージ
ウィンドウから入力イベントだけを受け取ります。`--read-resident`の`timeout`は
イベントが無かったことを示し、OSエラーではありません。

「マイクミュート」（function ID 45）は、Windowsに共通のマイクミュート仮想キーが
存在しないため、`SendInput`の仮想キーには変換しません。押下時にWindows Core Audio
の既定キャプチャendpointを取得し、`eCommunications`を優先して`GetMute`の反転を
`SetMute`へ渡した後、`GetMute`でreadbackします。成功すると、`REDSAMURAI_RECOVERY_LOG`
を設定したトレイでは次の形式が残ります。

```text
event=microphone_mute role=communications muted=true
```

もう一度押すと`muted=false`になります。実機確認ではWindowsのサウンド入力メーター
または録音アプリを併用してください。これは共有モードの既定キャプチャendpointを
対象にする実装であり、OEM固有のLED、排他的モードのアプリ、ハードウェア専用ミュート
を制御するものではありません。

**Git Bash:**

```sh
cmd //c msvc_cargo.bat build     # ビルド (//c は Git Bash 専用。PowerShellでは "/c" に)
cmd //c msvc_cargo.bat test      # 全テスト
cmd //c msvc_cargo.bat run       # テスト実行 (cargo run)
```

※ Git Bash から直接 `cargo` を実行すると Git の `/usr/bin/link.exe` が
   MSVC link.exe を shadow してリンクエラー (LNK1104等) になるため、必ずバッチ経由で。

## UI 自動化用アクセシビリティ識別子

カスタム描画されたボタン・チェックボックス・スライダー・メニュー行も AccessKit ノードとして
公開しています。通常ビルドでは `accesskit` feature が有効になり、各ノードの `AuthorId` は
Windows UI Automation の `AutomationId` として `red-samurai.` 接頭辞付きで安定します。
表示文字列に依存しないテストではこの ID を使ってください。互換クライアント向けに accessible
name にも `[a11y-id: red-samurai....]` を付加しています。

`verify-ui-accessibility.ps1` はリリース版を起動し、Device/Apply を押さずに UI Automation の
ID を検査する読み取り専用スモークです。PowerShell で `.\verify-ui-accessibility.ps1` を実行すると、
タブ切替後も ID が取得できることを確認し、実行ファイルの SHA-256 と確認した ID を JSON で出力します。

代表的な ID は `red-samurai.titlebar.close`、`red-samurai.device.status`、
`red-samurai.assignment.row.1`、`red-samurai.tab_0`、`red-samurai.profile_0`、
`red-samurai.bottom_6`、`red-samurai.dpi_vslider_0`、`red-samurai.light.custom-color`、
`red-samurai.macro.save`、`red-samurai.dialog.fire.ok` です。動的な行やステージの番号は
0 始まりです。AccessKit を無効にした埋め込みビルドでも、通常の egui 応答と名前のフォールバックは
維持されます。

## 検証済み項目

- `msvc_cargo.bat test` で Phase 1/1.5 の互換テストと Phase 2 境界テストを実行
- 現在の静的スイートは全295テスト（失敗0）
- **実ファイル byte-exact ラウンドトリップ**: 実際の
  `ドキュメント\RED SAMURAI 16400DPI Gaming Mouse\RSProfile1.pfd` を読んで保存すると
  バイト単位で同一 (`real_file_roundtrip_is_byte_exact`)
- UI スモークテスト: 起動スクリーンショットが純正スクショと一致

## 構成

| ファイル | 役割 | 実装 |
| --- | --- | --- |
| `src/ini.rs` | 順序保持 INI ドキュメント (CRLF 完全互換) | coordinator |
| `src/profile.rs` | RSProfile*.pfd モデル (DPI/LED/ボタン/ポーリング) | W1 (Orca task_a5601e951ecc) |
| `src/funcs.rs` / `src/menu.rs` | ボタン機能ラベル・割付メニュー | W2 (Orca task_52e08fc93748) |
| `src/assets.rs` / `src/ui_common.rs` | 純正スキン画像・共通ウィジェット | W3 (Orca task_931e19c6545b) |
| `src/app.rs` + `src/ui_*.rs` | アプリ殻・フレーム・4タブ | W4 (Orca task_dcb139409770) |
| `src/device.rs` | VID/PID + Usage Page/MI_02&COL02 の fail-closed HID transport | Phase2 W1 |
| `src/device_protocol.rs` | 16-byte / 64-byte / 0x400-byte report schema と観測済み command table | Phase2 W2 |
| `src/device_apply.rs` | Profile → ApplyPlan、dry-run warnings、明示的 apply seam | Phase2 W3 |
| `src/device_runtime.rs` | 公式プロセスなしのRust所有設定状態と再接続時の認可済み列再適用 | Phase2/3 |
| `src/resident.rs` | デバウンス、割付解決、マクロスケジューラ、InputSink境界 | Phase3 |
| `src/resident_platform.rs` | `MI_01` の厳密な列挙/解析、Windows SendInput | Phase3 |
| `src/resident_service.rs` | 常駐入力ループとProfile/マクロの組み合わせ | Phase3 |
| `src/tray.rs` | `--tray` 起動、通知領域メニュー、設定画面起動 | Phase4 |
| `src/phase4.rs` / `installer/` | HKCU Run、絶対パス検証、install/uninstall計画と実行スクリプト | Phase4 |

実装は Orca orchestration (run_b5acdbb88301) により 4 pi ワーカーで並列実行した。
各ワーカーの契約は `CONTRACT_*.md` に定義。

## Phase 2 の安全境界と現在の制限

- 対象デバイスは VID `04D9` / PID `FC55`、Usage Page `0xFFA0`、`MI_02&COL02`
  の設定 collection に限定します。別の collection や VID/PID だけの一致には
  fail-closed で応答します。
- 常駐入力は同じVID/PIDでも`MI_01`（hidapiの実パス末尾は`KBD`）、interface 1、
  Usage Page `0x0001` / Usage `0x0006`、9バイト入力だけを受け付けます。設定collectionを
  常駐入力として開いたり、別のHID collectionへフォールバックしたりしません。
- 通常の feature report は16 bytesです。USBPcap2で実測した `03/F3/20` は64 bytes、
  逆アセンブル上の `04/F3/C8` block候補は明示指定した場合だけ1024 bytesとして扱います。
  長さ不一致は送受信前にエラーにします。
- 逆アセンブルで観測できた command のうち、単独の profile field write として
  検証済みのものはありません。PollingRate、DPI table/current stage、LED mode
  /breath、ButtonFunc の未確定 sub-function (48/49/52/53) などは ApplyPlan の
  warning になり、推測した単独書き込みを生成しません。例外として、実機の
  125 Hz Applyで取得した **156件の完全な順序列**だけが、profile値 `8` に対する
  `VerifiedApplySequence` として認可されます。この完全列に限り、再接続後の
  A/B readbackで対応が一致した選択 `DPI=1/2` を、`02/F3/42` の16番目の
  report（byte[8]）へ置換できます。DPI stage/table の値や単独DPI reportは
  引き続き警告扱いです。
- `02/F2/2C` の状態問い合わせはApplyPlanに記録しますが、write列とは分離しており、
  `apply_*`から送信されません。コミット/resetや設定writeはUSBPcap等で対応付けを
  検証するまで実行しません。実機へ適用する前にdry-runの警告を確認してください。
- 完全Apply列の境界 `02/F5/00`→`02/F5/01` と列内の `02/F1/02,10,01,04,08` は
  観測済みですが、commit/resetの一般的な意味と単独送信は未確定です。認可列は
  個別frame APIを通らず、完全な順序・長さ・ヘッダ・完了数を再検証して送信します。
- `VERIFIED_COMMAND_MAPPINGS` は現在空のままです。UI相関や単独のbyte差分だけでは
  昇格せず、各候補には正確なA/B値が再接続後のreadbackでも持続する証拠と、完全な
  ordered Apply列（開始・終了・全report順序）の両方が揃うまで個別mappingとして登録しません。
  選択 `DPI=1/2` はこのregistryへの単独mapping登録ではなく、同じ証拠を持つ
  `VerifiedApplySequence` の限定的な置換として扱います。現時点のトークンは
  125 Hz の実機列全体を一つの不可分な認可単位として扱い、未観測のDPI値は
  fail-closedで拒否します。
- 昇格した純正UIの起動・Apply・profile切替を追加取得した結果、対象 `04D9:FC55`
  には `SET_REPORT` 317件（完全156件バースト×2＋短い5件）がありましたが、同じ対象の
  HID `GET_REPORT`（`0xA1/0x01`）は0件でした。これはこのUI経路で読み戻しを観測しなかった
  という負の証拠で、別経路の問い合わせや永続化の証明ではありません。
- UACを無効化した独立取得でも、起動直後は対象列挙5件のみでSET/GETとも0件、Applyは
  完全156件バースト1本とGET 0件でした。昇格状態に依存しない同じ観測です（詳細とハッシュは
  `ANALYSIS.md` §5.5）。
- 常駐 `hid.exe` を停止せずに78秒取得した再列挙用classic pcapでは、対象アドレス2に
  interrupt input 64件とゼロ長完了64件、列挙制御6件があり、SET/GETは0件でした。
  `RSProfile1.pfd` のサイズとSHA-256も不変で、PnP再起動は再起動待ち状態により拒否されました
  （詳細とハッシュは `ANALYSIS.md` §5.6）。
- 物理抜き差し後の公式125 Hz Applyでは、`155 x 16B + 1 x 64B` の156件すべてに
  zero-status completionがあり、Apply後のrestart/readbackでwire値 `08` が取得されました。
  この証拠は `captures/hardware-evidence-20260909T071720685Z-5d9fdff2/` と
  `ANALYSIS.md` §5.7 に固定し、125 Hz列の認可テンプレートにだけ使用しています。
- 選択DPIのRun A/B（`DPI=1` / `DPI=2`）では、Apply列と再接続後193件のGET
  responseが同じ1バイトだけ異なり、profile値とwire値の恒等対応 `1→0x01`,
  `2→0x02` を固定しました。比較表とPCAPハッシュは
  `../captures/dpi-reconnect-readback-comparison-20260911.md` にあります。

### 公式プロセスなしの運用範囲

Rustのトレイworkerは、入力の列挙・Raw Input監視・デバウンス・SendInput・切断復旧を
単独で所有します。明示的な「適用」では、完全な認可済みApply列だけをRustから送ります。
プロファイルに未検証項目が残っていても、既知のPollingRate/DPI列だけを分離して適用し、
未検証項目は変更しません。物理再接続後は同じ認可済み列を新しい設定handleへ一度だけ
再適用します。

選択DPIの40バイトReport-03応答は、公式プロセスが行っていた再接続後の追加コンテキストを
まだRustで再現していないため、単独GETでは証明しません。これは製品の入力・Apply・復旧の
動作条件ではなく、読み戻し証跡の追加ゲートです。短い8バイト応答はfail-closedで扱います。

## 重要な解析知見 (Phase 2 へ)

- `DPIStageValue` は **16 バイト/ステージ** `[enable u32][X le32][Y le32][flag u32=1]`
  (実ファイルで検証済み; コード 9/19/39/71/106)
- `LedColor1` は Windows COLORREF (0x00BBGGRR)
- デバイス通信: MI_02&COL02 (UsagePage 0xFFA0) への `[reportID][cmd][subcmd][params]`
  report。通常16B、対象キャプチャには64Bの `03/F3/20` が1件ある (ANALYSIS.md §5)。
  選択DPIのreadbackは report ID `03` の64B GET routeに対する40Bの
  `03/08/42/60/20` responseで、byte[8]だけを `DPI=1/2` として扱います。
- 追加の完全Applyキャプチャで PollingRate の PFD値/UI値と wire 値を固定した。対応は
  `125Hz (PFD 8) → 0x08`, `250Hz (PFD 4) → 0x04`, `500Hz (PFD 2) → 0x02`,
  `1000Hz (PFD 1) → 0x01` である。各操作は156件のreport列全体を送り、
  `02/F3/32`の14件目に差分が現れる。個別の `02/F3/32` は送信不可で、restart/readback
  まで揃った125 Hzの完全列だけが `VerifiedApplySequence` の認可対象です。
- Lightタブの5バーストでは、明るさが `02/F3/4F` byte[8] `02→03`、LEDモードと色が
  `02/F3/49`のbyte[8..12]へ現れた。DPIタブでは有効フラグが `02/F3/5C`、表示1000→2600
  の値差分が `02/F3/44` に現れ、Generalタブのボタン4変更では `02/F3/8E` byte[8]
  が `82→84` になった。いずれもUI相関までを固定し、全値への一般化はしていない。
- 追加キャプチャで現れた `02/F3/44`, `49`, `4F`, `5C`, `8E` は
  `src/device_protocol.rs` の観測済みヘッダ表にも登録したが、全て書き込み不可のままにしている。
- 未確定マッピング: DPIの全表示値とfield式、LedMode1/BreathState1の数値対応、色値の
  一意な対応、サブ機能ID (48/49/52/53)、`04/F3/C8`の対象デバイス帰属。

USBPcap2のハッシュと再現手順はリポジトリ直下の `ANALYSIS.md` §5.1–§5.5 に記載し、
`tests/usbpcap_evidence.rs`でWiresharkなしにpcapng/classic pcapのヘッダ、インターフェース、
完了レコード、各156件バースト、観測差分、readbackの負の証拠を固定化しています。\n