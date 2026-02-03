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
    property string statusMessage: "Click 'Connect' to connect to VPS"
    property bool isLoading: false
    property var selectedVideos: []
    property int downloadProgress: 0
    property string downloadSpeed: ""
    property bool isDownloading: false
    property bool isPaused: false
    property bool downloadComplete: false
    property bool downloadError: false

    VideoManager {
        id: videoManager
    }

    // Timer to poll download progress
    Timer {
        id: downloadPollTimer
        interval: 50
        repeat: true
        onTriggered: {
            downloadProgress = videoManager.get_download_progress()
            downloadSpeed = videoManager.get_download_speed()
            statusMessage = videoManager.get_status_message()
            isPaused = videoManager.get_is_paused()
            isDownloading = videoManager.get_is_downloading()
            downloadComplete = videoManager.get_download_complete()
            downloadError = videoManager.get_download_error()

            // Check if download finished
            if (!isDownloading && (downloadComplete || downloadError)) {
                downloadPollTimer.stop()
                isLoading = false

                if (downloadComplete) {
                    resultDialog.title = "Download Complete"
                    resultDialog.message = "Download finished successfully"
                    resultDialog.open()
                } else if (downloadError) {
                    resultDialog.title = "Download Failed"
                    resultDialog.message = "Download encountered an error"
                    resultDialog.open()
                }

                videoManager.reset_download_state()
            }
        }
    }

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
        var result = videoManager.connect_to_vps()
        statusMessage = videoManager.get_status_message()
        videoCount = videoManager.get_video_count()
        gridView.model = videoCount
        isLoading = videoManager.get_is_loading()
    }

    function doRefresh() {
        isLoading = true
        statusMessage = "Loading..."
        clearSelection()
        videoManager.refresh()
        statusMessage = videoManager.get_status_message()
        videoCount = videoManager.get_video_count()
        gridView.model = videoCount
        isLoading = videoManager.get_is_loading()
    }

    function doDownload(index, path) {
        isLoading = true
        isDownloading = true
        isPaused = false
        downloadComplete = false
        downloadError = false
        downloadProgress = 0
        downloadSpeed = ""
        videoManager.download_video(index, path)
        downloadPollTimer.start()
        // Download runs in background thread - timer will detect completion
    }

    function doBatchDownload(path) {
        if (selectedVideos.length === 0) {
            resultDialog.title = "No Selection"
            resultDialog.message = "Please select videos to download"
            resultDialog.open()
            return
        }

        isLoading = true
        var results = []
        var successCount = 0

        for (var i = 0; i < selectedVideos.length; i++) {
            var idx = selectedVideos[i]
            statusMessage = "Downloading " + (i + 1) + "/" + selectedVideos.length + "..."
            var result = videoManager.download_video(idx, path)
            if (result.indexOf("Downloaded") === 0) {
                successCount++
            }
            results.push(result)
        }

        statusMessage = videoManager.get_status_message()
        isLoading = false
        clearSelection()

        resultDialog.title = "Batch Download Complete"
        resultDialog.message = successCount + "/" + results.length + " videos downloaded successfully"
        resultDialog.open()
    }

    function doDelete(index) {
        isLoading = true
        var result = videoManager.delete_video(index)
        statusMessage = videoManager.get_status_message()
        videoCount = videoManager.get_video_count()
        gridView.model = videoCount
        isLoading = videoManager.get_is_loading()
        clearSelection()

        if (result.indexOf("Delete failed") !== -1) {
            resultDialog.title = "Delete Failed"
            resultDialog.message = result
            resultDialog.open()
        }
    }

    // File dialog for download location
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

    // Result dialog
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

    // Delete confirmation dialog
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

        onAccepted: {
            doDelete(deleteDialog.videoIndex)
        }
    }

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

                // YouTube icon (left)
                Rectangle {
                    width: 44
                    height: 32
                    color: "#ff0000"
                    radius: 6

                    Text {
                        anchors.centerIn: parent
                        text: "▶"
                        font.pixelSize: 18
                        color: "white"
                    }
                }

                // Title
                Text {
                    text: "Xero YouTube Video Manager"
                    font.pixelSize: 24
                    font.bold: true
                    color: "#e0e0e0"
                }

                // YouTube icon (right)
                Rectangle {
                    width: 44
                    height: 32
                    color: "#ff0000"
                    radius: 6

                    Text {
                        anchors.centerIn: parent
                        text: "▶"
                        font.pixelSize: 18
                        color: "white"
                    }
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
                    text: "Connect"
                    enabled: !isLoading
                    implicitWidth: 80

                    background: Rectangle {
                        color: parent.enabled ? (parent.hovered ? "#2ecc71" : "#27ae60") : "#555"
                        radius: 5
                    }

                    contentItem: Text {
                        text: parent.text
                        color: "white"
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }

                    onClicked: doConnect()
                }

                Rectangle {
                    width: 1
                    height: 30
                    color: "#3a3a5a"
                }

                Button {
                    text: "Download Selected (" + selectedVideos.length + ")"
                    enabled: !isLoading && selectedVideos.length > 0
                    implicitWidth: 160

                    background: Rectangle {
                        color: parent.enabled ? (parent.hovered ? "#9b59b6" : "#8e44ad") : "#555"
                        radius: 5
                    }

                    contentItem: Text {
                        text: parent.text
                        color: "white"
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }

                    onClicked: {
                        downloadDialog.batchMode = true
                        downloadDialog.open()
                    }
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
                        text: parent.text
                        color: "white"
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }

                    onClicked: clearSelection()
                }

                Item { Layout.fillWidth: true }

                // Download progress bar with speed and controls
                RowLayout {
                    visible: isDownloading
                    spacing: 6

                    // Progress bar
                    Rectangle {
                        width: 120
                        height: 28
                        color: "#2a2a4a"
                        radius: 4

                        Rectangle {
                            width: parent.width * (downloadProgress / 100)
                            height: parent.height
                            color: isPaused ? "#f39c12" : "#27ae60"
                            radius: 4

                            Behavior on width {
                                NumberAnimation { duration: 100 }
                            }
                        }

                        Text {
                            anchors.centerIn: parent
                            text: downloadProgress + "%"
                            color: "white"
                            font.pixelSize: 11
                            font.bold: true
                        }
                    }

                    // Speed display
                    Text {
                        text: downloadSpeed
                        color: "#3498db"
                        font.pixelSize: 11
                        font.bold: true
                        Layout.preferredWidth: 70
                    }

                    // Pause/Resume button
                    Button {
                        implicitWidth: 28
                        implicitHeight: 28

                        background: Rectangle {
                            color: parent.hovered ? (isPaused ? "#2ecc71" : "#f39c12") : (isPaused ? "#27ae60" : "#e67e22")
                            radius: 4
                        }

                        contentItem: Text {
                            text: isPaused ? "▶" : "⏸"
                            color: "white"
                            font.pixelSize: 12
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                        }

                        onClicked: {
                            if (isPaused) {
                                videoManager.resume_download()
                            } else {
                                videoManager.pause_download()
                            }
                        }

                        ToolTip.visible: hovered
                        ToolTip.text: isPaused ? "Resume" : "Pause"
                        ToolTip.delay: 300
                    }

                    // Cancel button
                    Button {
                        implicitWidth: 28
                        implicitHeight: 28

                        background: Rectangle {
                            color: parent.hovered ? "#c0392b" : "#e74c3c"
                            radius: 4
                        }

                        contentItem: Text {
                            text: "✕"
                            color: "white"
                            font.pixelSize: 14
                            font.bold: true
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                        }

                        onClicked: {
                            videoManager.cancel_download()
                        }

                        ToolTip.visible: hovered
                        ToolTip.text: "Cancel"
                        ToolTip.delay: 300
                    }
                }

                // Status message (hidden during download)
                Text {
                    text: statusMessage
                    color: "#a0a0a0"
                    font.pixelSize: 13
                    Layout.maximumWidth: 200
                    elide: Text.ElideRight
                    visible: !isDownloading
                }

                // Loading indicator
                BusyIndicator {
                    running: isLoading && !isDownloading
                    visible: isLoading && !isDownloading
                    width: 28
                    height: 28
                }

                // Refresh button (icon)
                Button {
                    enabled: !isLoading
                    implicitWidth: 36
                    implicitHeight: 36

                    background: Rectangle {
                        color: parent.enabled ? (parent.hovered ? "#3498db" : "#2980b9") : "#555"
                        radius: 5
                    }

                    contentItem: Text {
                        text: "⟳"
                        color: "white"
                        font.pixelSize: 20
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }

                    onClicked: doRefresh()

                    ToolTip.visible: hovered
                    ToolTip.text: "Refresh"
                    ToolTip.delay: 500
                }
            }
        }

        // Video grid - 3 columns x 2 rows visible
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
                    filename: videoManager.get_filename(index)
                    thumbnail: videoManager.get_thumbnail(index)
                    isSelected: selectedVideos.indexOf(index) !== -1

                    onDownloadClicked: {
                        downloadDialog.videoIndex = index
                        downloadDialog.batchMode = false
                        downloadDialog.open()
                    }

                    onDeleteClicked: {
                        deleteDialog.videoIndex = index
                        deleteDialog.filename = videoManager.get_filename(index)
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
                text: videoCount === 0 ? "Click 'Connect' then 'Refresh' to load videos" : ""
                color: "#606060"
                font.pixelSize: 16
                visible: videoCount === 0
            }
        }
    }
}
