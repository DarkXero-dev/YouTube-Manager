import QtQuick 2.15
import QtQuick.Controls 2.15

ApplicationWindow {
    visible: true
    width: 400
    height: 300
    title: "Test Window"

    Text {
        anchors.centerIn: parent
        text: "Hello World!"
        font.pixelSize: 24
    }
}
