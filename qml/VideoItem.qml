import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root

    property int videoIndex: 0
    property string filename: ""
    property string thumbnail: ""
    property bool isSelected: false

    signal downloadClicked()
    signal deleteClicked()
    signal playClicked()
    signal selectionChanged(int idx, bool checked)

    color: isSelected ? "#2a2a5f" : "#1e1e3f"
    radius: 8
    border.color: isSelected ? "#6a6aff" : (mouseArea.containsMouse ? "#4a4a8a" : "#2a2a5a")
    border.width: isSelected ? 3 : 2

    MouseArea {
        id: mouseArea
        anchors.fill: parent
        hoverEnabled: true
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 6

        // Thumbnail with checkbox overlay
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: "#0a0a1a"
            radius: 6
            clip: true

            Image {
                anchors.fill: parent
                anchors.margins: 2
                source: root.thumbnail
                fillMode: Image.PreserveAspectFit
                visible: root.thumbnail !== ""
            }

            // Video placeholder with gradient
            Rectangle {
                anchors.fill: parent
                visible: root.thumbnail === ""
                gradient: Gradient {
                    GradientStop { position: 0.0; color: "#2c3e50" }
                    GradientStop { position: 1.0; color: "#1a1a3a" }
                }

                Column {
                    anchors.centerIn: parent
                    spacing: 8

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: "🎬"
                        font.pixelSize: 36
                    }

                    Text {
                        width: root.width - 40
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: root.filename.replace(/\.[^/.]+$/, "")
                        color: "#a0a0a0"
                        font.pixelSize: 10
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                        maximumLineCount: 3
                        elide: Text.ElideRight
                    }
                }
            }

            // Checkbox in top-left corner
            CheckBox {
                id: selectCheckbox
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.margins: 5
                checked: root.isSelected

                indicator: Rectangle {
                    implicitWidth: 24
                    implicitHeight: 24
                    radius: 4
                    color: selectCheckbox.checked ? "#8e44ad" : "#2a2a4a"
                    border.color: selectCheckbox.checked ? "#9b59b6" : "#4a4a6a"
                    border.width: 2

                    Text {
                        anchors.centerIn: parent
                        text: "✓"
                        font.pixelSize: 16
                        font.bold: true
                        color: "white"
                        visible: selectCheckbox.checked
                    }
                }

                onClicked: {
                    root.selectionChanged(root.videoIndex, checked)
                }
            }

            // Play button overlay
            Rectangle {
                id: playButton
                anchors.centerIn: parent
                width: 50
                height: 50
                radius: 25
                color: playMouseArea.containsMouse ? "#cc0000" : "#aa000000"
                visible: mouseArea.containsMouse || playMouseArea.containsMouse

                Text {
                    anchors.centerIn: parent
                    text: "▶"
                    font.pixelSize: 24
                    color: "white"
                }

                MouseArea {
                    id: playMouseArea
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.playClicked()
                }
            }
        }

        // Filename
        Text {
            Layout.fillWidth: true
            text: root.filename
            color: "#e0e0e0"
            font.pixelSize: 11
            font.bold: true
            elide: Text.ElideMiddle
            maximumLineCount: 2
            wrapMode: Text.WrapAtWordBoundaryOrAnywhere

            ToolTip.visible: filenameMouseArea.containsMouse && root.filename.length > 30
            ToolTip.text: root.filename
            ToolTip.delay: 500

            MouseArea {
                id: filenameMouseArea
                anchors.fill: parent
                hoverEnabled: true
            }
        }

        // Buttons
        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Button {
                Layout.fillWidth: true
                text: "Download"
                implicitHeight: 28

                background: Rectangle {
                    color: parent.hovered ? "#2ecc71" : "#27ae60"
                    radius: 4
                }

                contentItem: Text {
                    text: parent.text
                    color: "white"
                    font.pixelSize: 11
                    font.bold: true
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                }

                onClicked: root.downloadClicked()
            }

            Button {
                Layout.fillWidth: true
                text: "Delete"
                implicitHeight: 28

                background: Rectangle {
                    color: parent.hovered ? "#e74c3c" : "#c0392b"
                    radius: 4
                }

                contentItem: Text {
                    text: parent.text
                    color: "white"
                    font.pixelSize: 11
                    font.bold: true
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                }

                onClicked: root.deleteClicked()
            }
        }
    }
}
