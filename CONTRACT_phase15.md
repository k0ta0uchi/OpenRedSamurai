# Phase 1.5 — 割付サブ機能・ダイアログ・マクロ 実装契約

対象スクショ: シングルキー設定/コンボキー設定/ベーシック/アドバンスト/メディア/マクロ/マクロ管理。
共通ルール: Phase 1 と同じ。自分の担当ファイルだけ編集。`cmd //c msvc_cargo.bat check` (Git Bash 例) を必ず通す。

## 解析済みの確定事項 (実ファイル/ldcfg.exe 逆アセンブルによる)

- 機能ID: ベーシック=16..23 (切り取り,コピー,貼り付け,すべて選択,検索,NEW,印刷,セーブ),
  アドバンスト=24..37 (スイッチウインドウ,クローズウインドウ,オープンウインドウ,実行,
  デスクトップを表示,ロックPC,ブラウザホーム,ブラウザ進む,ブラウザ戻る,ブラウザストップ,
  ブラウザリフレッシュ,ブラウザお気に入り,ブラウザ検索,メール),
  メディア=38..46 (再生/一時停止,ストップ,前のページ,次のページ,ボリュームアップ,
  ボリュームダウン,ミュート,マイクミュート,メディアプレーヤー),
  48/49=DPI+/− (公式プロファイルのFRONT5/FRONT6でライブ方向確認済み),
  52/53=DPI+/− (実プロファイルのボタン19/20で観測したID)。ボタン機能IDの
  デバイス側wire mappingは未検証。54=LEDモードスイッチ,
  1..15,100 は Phase1 の FUNCTION_STRING 通り。11=左ダブルクリック (メニュー順から確定)。
- プロファイルの選択 `DPI` 値は、公式の完全156-report Applyと再接続後GETの
  A/B証跡で `1→0x01`、`2→0x02` を確認済み。Rust側ではこの2値だけを
  `VerifiedApplySequence` の16番目 `02/F3/42` report byte[8]へ置換できる。
  単独report、DPI stage/table、その他のDPI値は引き続き未検証である。
  証跡は `../captures/dpi-reconnect-readback-comparison-20260911.md` に固定する。
- シングルキー: ButtonFunc=6, blob byte0 = **USB HID usage ID**
  ('A'..='Z'=0x04..0x1D, '1'..='0'=0x1E..0x27, Enter=0x28, Esc=0x29, Backspace=0x2A,
  Tab=0x2B, Space=0x2C, -=0x2D, ^=0x34(JIS @), F1..F12=0x3A..0x45, PrtScr=0x46, …)。
- コンボキー: ButtonFunc=7。本来のエンコードは未確定 → 我々の推定エンコード:
  KeyNumber=usage, LoopNumber=modifierマスク (bit0=CTRL,1=ALT,2=SHIFT,3=WIN)。Phase2で検証。
- マクロ: ButtonFunc=100, MacroName=マクロ名 (.pfdのButtonAssignedセクションに実フィールドあり)。

## マクロファイル形式 (実ファイル 777/888.MSMACRO, MacroSet.MSDB から確定)

- `MacroSet.MSDB`: INI (CRLF)。`[MACRO_LIST]` key 0..19 = マクロ名 (空スロットあり) +
  `[Setting]` Device=1, FirstOpen=1 (壊さず保持)。
- `<名前>.MSMACRO`: **Shift-JIS (cp932)** の INI。`[Setting]` のみ。
  - `MacroFilePath=<絶対パス>` (cp932、書込時は元の値をそのまま保持)
  - `LoopTime=1`, `DefDelayTime=10`, `LoopType=2`, `DelayType=0|2`, `Count=<レコード数>`
  - `MacroSetting_0=` hex (4バイトずつではない。**6バイト/レコード**):
    `[event u8 (0x84=押下,0x04=解放)][HID usage u8][0x00][0x06][delay_ms u16 LE]`
    レコード数 = Count。末尾を '0' でパディングし、**16進文字列の総長は元ファイルと同一**にする。
  - IO はバイト列→cp932でデコード→IniDoc→編集→cp932でエンコード→バイト列 (可逆)。
