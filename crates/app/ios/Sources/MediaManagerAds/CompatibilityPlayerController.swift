import UIKit
import VLCKit

/// Software-capable decoding independent of WKWebView. Security scope is held by storage.
@MainActor
final class CompatibilityPlayerController: UIViewController {
    private let url: URL
    private let finished: () -> Void
    private let player = VLCMediaPlayer()
    private let video = UIView()
    private let status = UILabel()
    private let pauseButton = UIButton(type: .system)
    private let slider = UISlider()
    private var timer: Timer?
    init(url: URL, finished: @escaping () -> Void) { self.url=url; self.finished=finished; super.init(nibName:nil,bundle:nil) }
    required init?(coder:NSCoder){fatalError("Programmatic controller")}
    override func viewDidLoad() {
        super.viewDidLoad();view.backgroundColor = .black
        status.text="Compatibility player";status.textColor = .white
        pauseButton.setTitle("Pause",for:.normal);pauseButton.addTarget(self,action:#selector(toggle),for:.touchUpInside)
        let close=UIButton(type:.system);close.setTitle("Close",for:.normal);close.addTarget(self,action:#selector(closePlayer),for:.touchUpInside)
        slider.accessibilityLabel="Playback position";slider.addTarget(self,action:#selector(seek),for:.valueChanged)
        let controls=UIStackView(arrangedSubviews:[pauseButton,close]);controls.distribution = .fillEqually
        let stack=UIStackView(arrangedSubviews:[video,status,slider,controls]);stack.axis = .vertical;stack.spacing=12;stack.translatesAutoresizingMaskIntoConstraints=false
        view.addSubview(stack);NSLayoutConstraint.activate([stack.leadingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.leadingAnchor,constant:16),stack.trailingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.trailingAnchor,constant:-16),stack.topAnchor.constraint(equalTo:view.safeAreaLayoutGuide.topAnchor,constant:16),stack.bottomAnchor.constraint(equalTo:view.safeAreaLayoutGuide.bottomAnchor,constant:-16),controls.heightAnchor.constraint(equalToConstant:48)])
        player.drawable=video;player.media=VLCMedia(url:url);player.play()
        timer=Timer.scheduledTimer(withTimeInterval:0.5,repeats:true){[weak self] _ in Task{@MainActor in guard let self else{return};self.slider.value=Float(self.player.position);self.status.text=self.player.state == .error ? "Playback failed. Check the file and connection." : "Compatibility player"}}
        NotificationCenter.default.addObserver(self,selector:#selector(background),name:UIApplication.didEnterBackgroundNotification,object:nil)
    }
    @objc private func toggle(){if player.isPlaying{player.pause();pauseButton.setTitle("Play",for:.normal)}else{player.play();pauseButton.setTitle("Pause",for:.normal)}}
    @objc private func seek(){if player.isSeekable{player.position=Double(slider.value)}}
    @objc private func background(){player.pause();pauseButton.setTitle("Play",for:.normal)}
    @objc private func closePlayer(){dismiss(animated:true)}
    override func viewDidDisappear(_ animated:Bool){super.viewDidDisappear(animated);timer?.invalidate();timer=nil;player.stop();player.drawable=nil;NotificationCenter.default.removeObserver(self);finished()}
}
