; Windows file-type registration for Arto.
;
; `dx bundle` renders its NSIS script from a fixed template that installs the
; binary, shortcuts and an uninstaller, and nothing else — so Windows never
; learns that Arto can open a document. Without the keys below Arto is missing
; from a Markdown file's "Open with" menu, from the "Choose another app" dialog
; and from Settings → Apps → Default apps. This is the registry spelling of what
; the macOS bundle declares as CFBundleDocumentTypes in extras/mac/Info.plist;
; keep the two extension lists in step.
;
; Wired up through `[bundle.windows.nsis] installer_hooks` in desktop/Dioxus.toml.
; The template `!include`s this file at top level, between the install and the
; uninstall section, so it may only define sections and macros — never bare
; statements. The installer shows no components page, so the section names here
; are internal.
;
; Everything is written under SHCTX, the installer's shell context: HKCU for the
; default per-user install, HKLM for a per-machine one. That puts it in the same
; hive as the Add/Remove Programs entry the template writes, so the uninstall
; below stays symmetric with it.

!define ARTO_EXE "arto.exe"
!define ARTO_MARKDOWN_PROGID "Arto.Markdown"
!define ARTO_TEXT_PROGID "Arto.Text"

; A ProgID is the file type itself: the name Explorer shows in the Type column,
; the icon it draws, and the command that opens it.
!macro ArtoWriteProgId ProgId TypeName
    WriteRegStr SHCTX "Software\Classes\${ProgId}" "" "${TypeName}"
    WriteRegStr SHCTX "Software\Classes\${ProgId}\DefaultIcon" "" "$INSTDIR\${ARTO_EXE},0"
    WriteRegStr SHCTX "Software\Classes\${ProgId}\shell\open" "FriendlyAppName" "Arto"
    WriteRegStr SHCTX "Software\Classes\${ProgId}\shell\open\command" "" '"$INSTDIR\${ARTO_EXE}" "%1"'
!macroend

; Offer Arto for an extension without taking it over: OpenWithProgids only adds
; a candidate to the "Open with" list, leaving whatever the user already has as
; the default. SupportedTypes is the other half — it is what the "Choose another
; app" dialog reads to decide that Arto is worth suggesting for this extension.
!macro ArtoRegisterExtension Extension ProgId
    WriteRegStr SHCTX "Software\Classes\${Extension}\OpenWithProgids" "${ProgId}" ""
    WriteRegStr SHCTX "Software\Classes\Applications\${ARTO_EXE}\SupportedTypes" "${Extension}" ""
!macroend

!macro ArtoUnregisterExtension Extension ProgId
    DeleteRegValue SHCTX "Software\Classes\${Extension}\OpenWithProgids" "${ProgId}"
!macroend

Section "Arto file associations"
    !insertmacro ArtoWriteProgId "${ARTO_MARKDOWN_PROGID}" "Markdown Document"
    !insertmacro ArtoWriteProgId "${ARTO_TEXT_PROGID}" "Text Document"

    ; The per-executable entry is what makes the "Open with" dialog list Arto
    ; under a readable name instead of the raw file name of the binary.
    WriteRegStr SHCTX "Software\Classes\Applications\${ARTO_EXE}" "FriendlyAppName" "Arto"
    WriteRegStr SHCTX "Software\Classes\Applications\${ARTO_EXE}\DefaultIcon" "" "$INSTDIR\${ARTO_EXE},0"
    WriteRegStr SHCTX "Software\Classes\Applications\${ARTO_EXE}\shell\open\command" "" '"$INSTDIR\${ARTO_EXE}" "%1"'

    !insertmacro ArtoRegisterExtension ".md" "${ARTO_MARKDOWN_PROGID}"
    !insertmacro ArtoRegisterExtension ".markdown" "${ARTO_MARKDOWN_PROGID}"
    !insertmacro ArtoRegisterExtension ".txt" "${ARTO_TEXT_PROGID}"
    !insertmacro ArtoRegisterExtension ".text" "${ARTO_TEXT_PROGID}"

    ; Settings → Default apps only lists an application that publishes a
    ; Capabilities key and points RegisteredApplications at it.
    WriteRegStr SHCTX "Software\Arto\Capabilities" "ApplicationName" "Arto"
    WriteRegStr SHCTX "Software\Arto\Capabilities" "ApplicationDescription" "A GitHub Markdown viewer"
    WriteRegStr SHCTX "Software\Arto\Capabilities\FileAssociations" ".md" "${ARTO_MARKDOWN_PROGID}"
    WriteRegStr SHCTX "Software\Arto\Capabilities\FileAssociations" ".markdown" "${ARTO_MARKDOWN_PROGID}"
    WriteRegStr SHCTX "Software\Arto\Capabilities\FileAssociations" ".txt" "${ARTO_TEXT_PROGID}"
    WriteRegStr SHCTX "Software\Arto\Capabilities\FileAssociations" ".text" "${ARTO_TEXT_PROGID}"
    WriteRegStr SHCTX "Software\RegisteredApplications" "Arto" "Software\Arto\Capabilities"

    ; The shell caches the association tables; without this notification the new
    ; entries only appear after Explorer is restarted.
    System::Call 'shell32.dll::SHChangeNotify(i 0x08000000, i 0, i 0, i 0)'
SectionEnd

Section "un.Arto file associations"
    !insertmacro ArtoUnregisterExtension ".md" "${ARTO_MARKDOWN_PROGID}"
    !insertmacro ArtoUnregisterExtension ".markdown" "${ARTO_MARKDOWN_PROGID}"
    !insertmacro ArtoUnregisterExtension ".txt" "${ARTO_TEXT_PROGID}"
    !insertmacro ArtoUnregisterExtension ".text" "${ARTO_TEXT_PROGID}"

    DeleteRegKey SHCTX "Software\Classes\${ARTO_MARKDOWN_PROGID}"
    DeleteRegKey SHCTX "Software\Classes\${ARTO_TEXT_PROGID}"
    DeleteRegKey SHCTX "Software\Classes\Applications\${ARTO_EXE}"

    DeleteRegValue SHCTX "Software\RegisteredApplications" "Arto"
    DeleteRegKey SHCTX "Software\Arto"

    System::Call 'shell32.dll::SHChangeNotify(i 0x08000000, i 0, i 0, i 0)'
SectionEnd
