import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import VideoManager 1.0

ApplicationWindow {
    id: window
    visible: true
    width: 1100
    height: 750
    minimumWidth: 900
    minimumHeight: 600
    title: "Xero YouTube Video Manager"

    color: "#1a1a2e"

    property int videoCount: 0
    property string statusMessage: "Connecting..."
    property bool isLoading: false
    property var selectedVideos: []
    property int downloadProgress: 0
    property string downloadSpeed: ""
    property bool isDownloading: false
    property bool isPaused: false
    property bool downloadComplete: false
    property bool downloadError: false
    property int thumbnailVersion: 0  // incremented when new thumbnails arrive
    property bool vpsConnected: false // polled by timer; drives Browse VPS button

    VideoManager {
        id: videoManager
    }

    // Clipboard helper (hidden TextEdit used to copy text)
    TextEdit {
        id: clipHelper
        visible: false
    }

    function copyToClipboard(text) {
        clipHelper.text = text
        clipHelper.selectAll()
        clipHelper.copy()
        clipHelper.text = ""
    }

    Component.onCompleted: {
        // Show crash report from previous session if one exists
        var crashLog = videoManager.get_crash_log()
        if (crashLog !== "") {
            crashDialog.crashText = crashLog
            crashDialog.open()
        }

        // If no SSH key is configured/found, prompt for credentials first
        if (!videoManager.has_valid_key()) {
            credentialsDialog.isRequired = true
            credStatusLabel.text = ""
            credSetupBtn.enabled = true
            credPassField.text = ""
            credentialsDialog.open()
        } else {
            doConnect()
        }
    }

    // ── Download progress polling ──────────────────────────────────────────────
    Timer {
        id: downloadPollTimer
        interval: 50
        repeat: true
        onTriggered: {
            downloadProgress = videoManager.get_download_progress()
            downloadSpeed    = videoManager.get_download_speed()
            statusMessage    = videoManager.get_status_message()
            isPaused         = videoManager.get_is_paused()
            isDownloading    = videoManager.get_is_downloading()
            downloadComplete = videoManager.get_download_complete()
            downloadError    = videoManager.get_download_error()

            if (!isDownloading && (downloadComplete || downloadError)) {
                downloadPollTimer.stop()
                isLoading = false

                if (downloadComplete) {
                    resultDialog.title   = "Download Complete"
                    resultDialog.message = "Download finished successfully"
                    resultDialog.open()
                } else if (downloadError) {
                    resultDialog.title   = "Download Failed"
                    resultDialog.message = "Download encountered an error"
                    resultDialog.open()
                }

                videoManager.reset_download_state()
            }
        }
    }

    // ── Thumbnail & error polling ─────────────────────────────────────────────
    Timer {
        id: bgPollTimer
        interval: 300
        repeat: true
        running: true
        onTriggered: {
            // Track connection state for reactive bindings
            vpsConnected = videoManager.is_connected()

            // Pull any newly loaded thumbnails from the background thread
            var newThumbs = videoManager.poll_thumbnails()
            if (newThumbs > 0) {
                thumbnailVersion++
            }

            // Show any backend errors in the error dialog
            if (videoManager.has_error()) {
                errorDialog.errorText = videoManager.get_last_error()
                if (!errorDialog.visible) {
                    errorDialog.open()
                }
            }
        }
    }

    // ── QML functions ─────────────────────────────────────────────────────────

    function updateSelection(index, checked) {
        var newSelection = selectedVideos.slice()
        if (checked) {
            if (newSelection.indexOf(index) === -1) {
                newSelection.push(index)
            }
        } else {
            var idx = newSelection.indexOf(index)
            if (idx !== -1) {
                newSelection.splice(idx, 1)
            }
        }
        selectedVideos = newSelection
    }

    function clearSelection() {
        selectedVideos = []
    }

    function doConnect() {
        isLoading = true
        statusMessage = "Connecting..."
        videoManager.connect_to_vps()
        statusMessage = videoManager.get_status_message()
        videoCount    = videoManager.get_video_count()
        gridView.model = videoCount
        isLoading     = videoManager.get_is_loading()
    }

    function doRefresh() {
        isLoading = true
        statusMessage = "Loading..."
        clearSelection()
        videoManager.refresh()
        statusMessage  = videoManager.get_status_message()
        videoCount     = videoManager.get_video_count()
        gridView.model = videoCount
        isLoading      = videoManager.get_is_loading()
    }

    function doDownload(index, path) {
        isLoading        = true
        isDownloading    = true
        isPaused         = false
        downloadComplete = false
        downloadError    = false
        downloadProgress = 0
        downloadSpeed    = ""
        videoManager.download_video(index, path)
        downloadPollTimer.start()
    }

    function doBatchDownload(path) {
        if (selectedVideos.length === 0) {
            resultDialog.title   = "No Selection"
            resultDialog.message = "Please select videos to download"
            resultDialog.open()
            return
        }

        isLoading = true
        for (var i = 0; i < selectedVideos.length; i++) {
            statusMessage = "Downloading " + (i + 1) + "/" + selectedVideos.length + "..."
            videoManager.download_video(selectedVideos[i], path)
        }

        statusMessage = videoManager.get_status_message()
        isLoading     = false
        clearSelection()

        resultDialog.title   = "Batch Download Started"
        resultDialog.message = selectedVideos.length + " download(s) queued"
        resultDialog.open()
    }

    function doBatchDelete() {
        if (selectedVideos.length === 0) {
            resultDialog.title   = "No Selection"
            resultDialog.message = "Please select videos to delete"
            resultDialog.open()
            return
        }

        isLoading = true
        var indicesCsv = selectedVideos.join(",")
        var result = videoManager.batch_delete_videos(indicesCsv)
        statusMessage  = videoManager.get_status_message()
        videoCount     = videoManager.get_video_count()
        gridView.model = videoCount
        isLoading      = videoManager.get_is_loading()
        clearSelection()

        resultDialog.title   = "Batch Delete Complete"
        resultDialog.message = result
        resultDialog.open()
    }

    function doDelete(index) {
        isLoading = true
        var result = videoManager.delete_video(index)
        statusMessage  = videoManager.get_status_message()
        videoCount     = videoManager.get_video_count()
        gridView.model = videoCount
        isLoading      = videoManager.get_is_loading()
        clearSelection()

        if (result.indexOf("Delete failed") !== -1) {
            resultDialog.title   = "Delete Failed"
            resultDialog.message = result
            resultDialog.open()
        }
    }

    // ── Dialogs ───────────────────────────────────────────────────────────────

    FolderDialog {
        id: downloadDialog
        title: "Select Download Location"

        property int videoIndex: -1
        property bool batchMode: false

        onAccepted: {
            var path = downloadDialog.selectedFolder.toString().replace("file://", "")
            if (batchMode) {
                doBatchDownload(path)
            } else {
                doDownload(videoIndex, path)
            }
        }
    }

    // Generic result dialog
    Dialog {
        id: resultDialog
        anchors.centerIn: parent
        width: 400
        property string message: ""
        title: "Result"

        Label {
            text: resultDialog.message
            wrapMode: Text.WordWrap
            width: parent.width
            color: "#e0e0e0"
        }
        standardButtons: Dialog.Ok
    }

    // Single-video delete confirmation
    Dialog {
        id: deleteDialog
        anchors.centerIn: parent
        width: 400
        title: "Confirm Delete"

        property int videoIndex: -1
        property string filename: ""

        Label {
            text: "Are you sure you want to delete:\n" + deleteDialog.filename + "?"
            wrapMode: Text.WordWrap
            width: parent.width
            color: "#e0e0e0"
        }
        standardButtons: Dialog.Yes | Dialog.No
        onAccepted: doDelete(deleteDialog.videoIndex)
    }

    // Batch delete confirmation
    Dialog {
        id: batchDeleteDialog
        anchors.centerIn: parent
        width: 400
        title: "Confirm Batch Delete"

        Label {
            text: "Delete " + selectedVideos.length + " selected video(s) from the VPS?\nThis cannot be undone."
            wrapMode: Text.WordWrap
            width: parent.width
            color: "#e0e0e0"
        }
        standardButtons: Dialog.Yes | Dialog.No
        onAccepted: doBatchDelete()
    }

    // ── Credentials dialog ────────────────────────────────────────────────────
    Dialog {
        id: credentialsDialog
        anchors.centerIn: parent
        width: 460
        title: isRequired ? "VPS Setup Required" : "Settings"
        modal: true

        property bool isRequired: false
        closePolicy: isRequired ? Popup.NoAutoClose : (Popup.CloseOnEscape | Popup.CloseOnPressOutside)

        // Deferred setup timer — lets the UI repaint "Connecting..." before blocking
        Timer {
            id: setupTimer
            interval: 80
            repeat: false
            onTriggered: {
                var result = videoManager.setup_credentials(
                    credHostField.text,
                    credUserField.text,
                    credPassField.text
                )
                if (result === "success") {
                    credentialsDialog.close()
                    doConnect()
                } else {
                    credStatusLabel.text = result
                    credStatusLabel.color = "#e74c3c"
                    credSetupBtn.enabled = true
                    credCancelBtn.enabled = true
                    credPassField.text = ""
                }
            }
        }

        ColumnLayout {
            width: credentialsDialog.availableWidth
            spacing: 12

            Label {
                text: "Enter your VPS credentials. An SSH key will be generated\n" +
                      "and installed automatically — your password is never stored."
                color: "#a0c0ff"
                font.pixelSize: 12
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }

            Label { text: "VPS IP / Hostname:"; color: "#c0c0c0"; font.pixelSize: 12 }
            TextField {
                id: credHostField
                Layout.fillWidth: true
                text: videoManager.get_config_host()
                placeholderText: "192.168.1.1"
                color: "#e0e0e0"
                background: Rectangle { color: "#2a2a4a"; radius: 4 }
            }

            Label { text: "SSH Username:"; color: "#c0c0c0"; font.pixelSize: 12 }
            TextField {
                id: credUserField
                Layout.fillWidth: true
                text: videoManager.get_config_user()
                placeholderText: "username"
                color: "#e0e0e0"
                background: Rectangle { color: "#2a2a4a"; radius: 4 }
            }

            Label { text: "Password:"; color: "#c0c0c0"; font.pixelSize: 12 }
            TextField {
                id: credPassField
                Layout.fillWidth: true
                echoMode: TextInput.Password
                placeholderText: "VPS password"
                color: "#e0e0e0"
                background: Rectangle { color: "#2a2a4a"; radius: 4 }
            }

            // ── Videos directory (browseable after connecting) ─────────────
            Rectangle {
                Layout.fillWidth: true
                height: 1
                color: "#3a3a5a"
            }

            RowLayout {
                Layout.fillWidth: true

                Label {
                    text: "Videos Directory on VPS:"
                    color: "#c0c0c0"
                    font.pixelSize: 12
                }

                Item { Layout.fillWidth: true }

                Label {
                    text: vpsConnected ? "" : "(connect first to browse)"
                    color: "#505070"
                    font.pixelSize: 10
                    visible: !vpsConnected
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Rectangle {
                    Layout.fillWidth: true
                    height: 34
                    color: "#2a2a4a"
                    radius: 4

                    Text {
                        id: videosDirDisplay
                        anchors.fill: parent
                        anchors.margins: 8
                        text: videoManager.get_config_videos_dir()
                        color: "#e0e0e0"
                        verticalAlignment: Text.AlignVCenter
                        elide: Text.ElideLeft
                        font.family: "monospace"
                        font.pixelSize: 12
                    }
                }

                Button {
                    text: "Browse"
                    implicitHeight: 34
                    enabled: vpsConnected
                    background: Rectangle {
                        color: parent.enabled
                            ? (parent.hovered ? "#3498db" : "#2980b9")
                            : "#444"
                        radius: 4
                    }
                    contentItem: Text {
                        text: parent.text; color: parent.enabled ? "white" : "#707070"
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        font.pixelSize: 12
                    }
                    onClicked: remoteBrowserDialog.open()
                }
            }

            // Status / error feedback
            Label {
                id: credStatusLabel
                text: ""
                color: "#a0c0ff"
                font.pixelSize: 12
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
                visible: text !== ""
            }

            // Buttons row
            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Item { Layout.fillWidth: true }

                Button {
                    id: credCancelBtn
                    text: "Cancel"
                    visible: !credentialsDialog.isRequired
                    implicitWidth: 80
                    background: Rectangle {
                        color: parent.hovered ? "#555" : "#444"; radius: 4
                    }
                    contentItem: Text {
                        text: parent.text; color: "#c0c0c0"
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    onClicked: credentialsDialog.close()
                }

                Button {
                    id: credSetupBtn
                    text: "Setup & Connect"
                    implicitWidth: 130
                    background: Rectangle {
                        color: parent.enabled
                            ? (parent.hovered ? "#2ecc71" : "#27ae60")
                            : "#555"
                        radius: 4
                    }
                    contentItem: Text {
                        text: parent.text; color: "white"; font.bold: true
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    onClicked: {
                        credStatusLabel.text = "Generating SSH key and connecting to VPS..."
                        credStatusLabel.color = "#a0c0ff"
                        credSetupBtn.enabled = false
                        credCancelBtn.enabled = false
                        setupTimer.start()
                    }
                }
            }
        }

        // No standardButtons — handled manually above
    }

    // ── Remote VPS filesystem browser ─────────────────────────────────────────
    Dialog {
        id: remoteBrowserDialog
        anchors.centerIn: parent
        width: 520
        height: 500
        title: "Browse VPS — Select Videos Directory"
        modal: true

        property string currentPath: "/"

        function navigateTo(path) {
            var raw = videoManager.list_remote_dirs(path)
            if (raw.startsWith("ERROR:")) {
                browserStatus.text = raw.substring(6)
                return
            }
            currentPath = path
            browserStatus.text = ""
            dirModel.clear()
            if (raw !== "") {
                var names = raw.split("\n")
                for (var i = 0; i < names.length; i++) {
                    if (names[i] !== "")
                        dirModel.append({ name: names[i] })
                }
            }
        }

        function goUp() {
            if (currentPath === "/" || currentPath === "") return
            var trimmed = currentPath.replace(/\/+$/, "")
            var last = trimmed.lastIndexOf("/")
            var parent = last <= 0 ? "/" : trimmed.substring(0, last)
            navigateTo(parent)
        }

        function joinPath(base, name) {
            return base.replace(/\/+$/, "") + "/" + name
        }

        onAboutToShow: {
            // Start at the currently configured videos dir (strip trailing /)
            var start = videoManager.get_config_videos_dir().replace(/\/+$/, "") || "/"
            navigateTo(start)
        }

        ColumnLayout {
            anchors.fill: parent
            spacing: 8

            // ── Path bar ───────────────────────────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Button {
                    text: "↑ Up"
                    implicitWidth: 60
                    implicitHeight: 32
                    enabled: remoteBrowserDialog.currentPath !== "/"
                    background: Rectangle {
                        color: parent.enabled ? (parent.hovered ? "#3498db" : "#2980b9") : "#444"
                        radius: 4
                    }
                    contentItem: Text {
                        text: parent.text; color: "white"
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    onClicked: remoteBrowserDialog.goUp()
                }

                Rectangle {
                    Layout.fillWidth: true
                    height: 32
                    color: "#0f0f23"
                    radius: 4

                    Text {
                        anchors.fill: parent
                        anchors.margins: 8
                        text: remoteBrowserDialog.currentPath
                        color: "#e0e0e0"
                        verticalAlignment: Text.AlignVCenter
                        elide: Text.ElideLeft
                        font.family: "monospace"
                        font.pixelSize: 12
                    }
                }
            }

            // ── Directory list ─────────────────────────────────────────────
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                color: "#0f0f23"
                radius: 6
                clip: true

                ListView {
                    id: dirListView
                    anchors.fill: parent
                    anchors.margins: 4
                    clip: true
                    model: ListModel { id: dirModel }

                    delegate: ItemDelegate {
                        width: dirListView.width
                        height: 36

                        background: Rectangle {
                            color: parent.hovered ? "#1e2a4a" : "transparent"
                            radius: 4
                        }

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10
                            anchors.rightMargin: 10
                            spacing: 8

                            Text {
                                text: "📁"
                                font.pixelSize: 15
                            }
                            Text {
                                text: model.name
                                color: "#e0e0e0"
                                font.pixelSize: 13
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                            }
                            Text {
                                text: "▶"
                                color: "#505070"
                                font.pixelSize: 11
                            }
                        }

                        onDoubleClicked: {
                            remoteBrowserDialog.navigateTo(
                                remoteBrowserDialog.joinPath(remoteBrowserDialog.currentPath, model.name)
                            )
                        }
                    }

                    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                }

                // Empty hint
                Text {
                    anchors.centerIn: parent
                    text: "No subdirectories"
                    color: "#404060"
                    font.pixelSize: 14
                    visible: dirModel.count === 0 && browserStatus.text === ""
                }
            }

            // ── Error / status ─────────────────────────────────────────────
            Label {
                id: browserStatus
                text: ""
                color: "#e74c3c"
                font.pixelSize: 12
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
                visible: text !== ""
            }

            // ── Bottom bar: selected path + confirm ────────────────────────
            Rectangle {
                Layout.fillWidth: true
                height: 1
                color: "#3a3a5a"
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Column {
                    Layout.fillWidth: true
                    spacing: 2

                    Label {
                        text: "Double-click to navigate · Click to select:"
                        color: "#606070"
                        font.pixelSize: 10
                    }
                    Label {
                        text: remoteBrowserDialog.currentPath
                        color: "#a0c0ff"
                        font.pixelSize: 12
                        font.family: "monospace"
                        elide: Text.ElideLeft
                        width: parent.width
                    }
                }

                Button {
                    text: "Select"
                    implicitWidth: 80
                    implicitHeight: 34
                    background: Rectangle {
                        color: parent.hovered ? "#2ecc71" : "#27ae60"
                        radius: 4
                    }
                    contentItem: Text {
                        text: parent.text; color: "white"; font.bold: true
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    onClicked: {
                        var path = remoteBrowserDialog.currentPath
                        if (!path.endsWith("/")) path += "/"
                        videoManager.save_videos_dir(path)
                        // Update the display label in the credentials dialog
                        videosDirDisplay.text = path
                        remoteBrowserDialog.close()
                    }
                }
            }
        }
    }

    // ── Crash report dialog (previous session crashed) ────────────────────────
    Dialog {
        id: crashDialog
        anchors.centerIn: parent
        width: 520
        title: "Previous Session Crashed"

        property string crashText: ""

        ColumnLayout {
            width: crashDialog.availableWidth
            spacing: 10

            Label {
                text: "The previous session ended unexpectedly. Error details:"
                color: "#e74c3c"
                font.bold: true
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
            }

            ScrollView {
                Layout.fillWidth: true
                Layout.preferredHeight: 180
                clip: true

                TextArea {
                    id: crashTextArea
                    text: crashDialog.crashText
                    readOnly: true
                    wrapMode: TextArea.Wrap
                    color: "#e0e0e0"
                    font.family: "monospace"
                    font.pixelSize: 11
                    background: Rectangle { color: "#0f0f23"; radius: 4 }
                }
            }

            Button {
                text: "Copy Error to Clipboard"
                Layout.alignment: Qt.AlignHCenter
                background: Rectangle {
                    color: parent.hovered ? "#7f8c8d" : "#606060"
                    radius: 4
                }
                contentItem: Text {
                    text: parent.text; color: "white"
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                }
                onClicked: copyToClipboard(crashDialog.crashText)
            }
        }

        standardButtons: Dialog.Ok
    }

    // ── Runtime error dialog ──────────────────────────────────────────────────
    Dialog {
        id: errorDialog
        anchors.centerIn: parent
        width: 460
        title: "Error"

        property string errorText: ""

        ColumnLayout {
            width: errorDialog.availableWidth
            spacing: 10

            Label {
                text: errorDialog.errorText
                wrapMode: Text.WordWrap
                color: "#e0e0e0"
                Layout.fillWidth: true
            }

            Button {
                text: "Copy to Clipboard"
                Layout.alignment: Qt.AlignHCenter
                background: Rectangle {
                    color: parent.hovered ? "#7f8c8d" : "#606060"
                    radius: 4
                }
                contentItem: Text {
                    text: parent.text; color: "white"
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                }
                onClicked: copyToClipboard(errorDialog.errorText)
            }
        }

        standardButtons: Dialog.Ok
    }

    // ── Main layout ───────────────────────────────────────────────────────────
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 15
        spacing: 15

        // Header
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 60
            color: "#16213e"
            radius: 10

            RowLayout {
                anchors.centerIn: parent
                spacing: 15

                Rectangle {
                    width: 44; height: 32
                    color: "#ff0000"; radius: 6
                    Text { anchors.centerIn: parent; text: "▶"; font.pixelSize: 18; color: "white" }
                }

                Text {
                    text: "Xero YouTube Video Manager"
                    font.pixelSize: 24; font.bold: true; color: "#e0e0e0"
                }

                Rectangle {
                    width: 44; height: 32
                    color: "#ff0000"; radius: 6
                    Text { anchors.centerIn: parent; text: "▶"; font.pixelSize: 18; color: "white" }
                }
            }
        }

        // Control bar
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 50
            color: "#16213e"
            radius: 8

            RowLayout {
                anchors.fill: parent
                anchors.margins: 10
                spacing: 10

                Button {
                    text: "Download Selected (" + selectedVideos.length + ")"
                    enabled: !isLoading && selectedVideos.length > 0
                    implicitWidth: 160

                    background: Rectangle {
                        color: parent.enabled ? (parent.hovered ? "#9b59b6" : "#8e44ad") : "#555"
                        radius: 5
                    }
                    contentItem: Text {
                        text: parent.text; color: "white"
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    onClicked: {
                        downloadDialog.batchMode = true
                        downloadDialog.open()
                    }
                }

                Button {
                    text: "Delete Selected (" + selectedVideos.length + ")"
                    enabled: !isLoading && selectedVideos.length > 0
                    implicitWidth: 160

                    background: Rectangle {
                        color: parent.enabled ? (parent.hovered ? "#e74c3c" : "#c0392b") : "#555"
                        radius: 5
                    }
                    contentItem: Text {
                        text: parent.text; color: "white"
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    onClicked: batchDeleteDialog.open()
                }

                Button {
                    text: "Clear"
                    enabled: selectedVideos.length > 0
                    implicitWidth: 60

                    background: Rectangle {
                        color: parent.enabled ? (parent.hovered ? "#7f8c8d" : "#95a5a6") : "#555"
                        radius: 5
                    }
                    contentItem: Text {
                        text: parent.text; color: "white"
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    onClicked: clearSelection()
                }

                Item { Layout.fillWidth: true }

                // Download progress bar + controls
                RowLayout {
                    visible: isDownloading
                    spacing: 6

                    Rectangle {
                        width: 120; height: 28
                        color: "#2a2a4a"; radius: 4

                        Rectangle {
                            width: parent.width * (downloadProgress / 100)
                            height: parent.height
                            color: isPaused ? "#f39c12" : "#27ae60"
                            radius: 4
                            Behavior on width { NumberAnimation { duration: 100 } }
                        }
                        Text {
                            anchors.centerIn: parent
                            text: downloadProgress + "%"
                            color: "white"; font.pixelSize: 11; font.bold: true
                        }
                    }

                    Text {
                        text: downloadSpeed
                        color: "#3498db"; font.pixelSize: 11; font.bold: true
                        Layout.preferredWidth: 70
                    }

                    Button {
                        implicitWidth: 28; implicitHeight: 28
                        background: Rectangle {
                            color: parent.hovered ? (isPaused ? "#2ecc71" : "#f39c12")
                                                 : (isPaused ? "#27ae60" : "#e67e22")
                            radius: 4
                        }
                        contentItem: Text {
                            text: isPaused ? "▶" : "⏸"; color: "white"
                            font.pixelSize: 12
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                        }
                        onClicked: isPaused ? videoManager.resume_download() : videoManager.pause_download()
                        ToolTip.visible: hovered
                        ToolTip.text: isPaused ? "Resume" : "Pause"
                        ToolTip.delay: 300
                    }

                    Button {
                        implicitWidth: 28; implicitHeight: 28
                        background: Rectangle {
                            color: parent.hovered ? "#c0392b" : "#e74c3c"; radius: 4
                        }
                        contentItem: Text {
                            text: "✕"; color: "white"; font.pixelSize: 14; font.bold: true
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                        }
                        onClicked: videoManager.cancel_download()
                        ToolTip.visible: hovered
                        ToolTip.text: "Cancel"
                        ToolTip.delay: 300
                    }
                }

                Text {
                    text: statusMessage
                    color: "#a0a0a0"; font.pixelSize: 13
                    Layout.maximumWidth: 200
                    elide: Text.ElideRight
                    visible: !isDownloading
                }

                BusyIndicator {
                    running: isLoading && !isDownloading
                    visible: isLoading && !isDownloading
                    width: 28; height: 28
                }

                // Refresh button
                Button {
                    enabled: !isLoading
                    implicitWidth: 36; implicitHeight: 36
                    background: Rectangle {
                        color: parent.enabled ? (parent.hovered ? "#3498db" : "#2980b9") : "#555"
                        radius: 5
                    }
                    contentItem: Text {
                        text: "⟳"; color: "white"; font.pixelSize: 20
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    onClicked: doRefresh()
                    ToolTip.visible: hovered
                    ToolTip.text: "Refresh"
                    ToolTip.delay: 500
                }

                // Settings button
                Button {
                    implicitWidth: 36; implicitHeight: 36
                    background: Rectangle {
                        color: parent.hovered ? "#e67e22" : "#d35400"; radius: 5
                    }
                    contentItem: Text {
                        text: "⚙"; color: "white"; font.pixelSize: 18
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    onClicked: {
                        credentialsDialog.isRequired = false
                        credStatusLabel.text = ""
                        credSetupBtn.enabled = true
                        credPassField.text = ""
                        videosDirDisplay.text = videoManager.get_config_videos_dir()
                        credentialsDialog.open()
                    }
                    ToolTip.visible: hovered
                    ToolTip.text: "Credentials / Settings"
                    ToolTip.delay: 500
                }
            }
        }

        // Video grid
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: "#0f0f23"
            radius: 10

            GridView {
                id: gridView
                anchors.fill: parent
                anchors.margins: 10
                cellWidth: (parent.width - 20) / 3
                cellHeight: (parent.height - 20) / 2
                clip: true
                model: 0

                delegate: VideoItem {
                    width: gridView.cellWidth - 10
                    height: gridView.cellHeight - 10

                    videoIndex: index
                    filename:   videoManager.get_filename(index)
                    // Re-evaluated whenever thumbnailVersion increments
                    thumbnail:  thumbnailVersion >= 0 ? videoManager.get_thumbnail(index) : ""
                    isSelected: selectedVideos.indexOf(index) !== -1

                    onDownloadClicked: {
                        downloadDialog.videoIndex = index
                        downloadDialog.batchMode  = false
                        downloadDialog.open()
                    }
                    onDeleteClicked: {
                        deleteDialog.videoIndex = index
                        deleteDialog.filename   = videoManager.get_filename(index)
                        deleteDialog.open()
                    }
                    onPlayClicked: {
                        videoManager.play_video(index)
                        statusMessage = videoManager.get_status_message()
                    }
                    onSelectionChanged: function(idx, checked) {
                        updateSelection(idx, checked)
                    }
                }

                ScrollBar.vertical: ScrollBar {
                    policy: ScrollBar.AsNeeded
                }
            }

            // Empty state
            Text {
                anchors.centerIn: parent
                text: isLoading ? "Connecting to VPS..."
                                : (videoCount === 0 ? "No videos found — check credentials or refresh." : "")
                color: "#606060"
                font.pixelSize: 16
                visible: videoCount === 0
            }
        }
    }
}
