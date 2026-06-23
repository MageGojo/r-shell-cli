import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    let windowFrame = self.frame
    self.contentViewController = flutterViewController
    self.setFrame(windowFrame, display: true)

    // R-Shell：最小与初始窗口尺寸（对齐设计规范，最小 1040×640）。
    self.minSize = NSSize(width: 1040, height: 640)
    self.setContentSize(NSSize(width: 1200, height: 780))
    self.center()

    RegisterGeneratedPlugins(registry: flutterViewController)

    super.awakeFromNib()
  }
}
