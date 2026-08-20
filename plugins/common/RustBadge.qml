import QtQuick
import qs.Commons
import qs.Ui

BorderSurface {
  id: root

  property bool highlighted: true
  property color foreground: Color.foreground
  property color accent: "#ff6a1a"
  property string fontFamily: Style.font.family

  implicitWidth: mark.implicitWidth + Style.space(10)
  implicitHeight: mark.implicitHeight + Style.space(4)
  color: highlighted ? accent : "transparent"
  borderSpec: Border.controlSpec(highlighted ? "selected" : "normal", foreground, accent)
  radius: Style.cornerRadius

  Text {
    id: mark
    anchors.centerIn: parent
    text: ""
    color: root.highlighted ? "#1a0d08" : root.foreground
    font.family: root.fontFamily
    font.pixelSize: Style.font.title
    font.bold: true
  }
}
