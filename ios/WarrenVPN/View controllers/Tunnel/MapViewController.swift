//
//  MapViewController.swift
//  MullvadVPN
//
//  Created by pronebird on 03/01/2023.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import CoreImage
import Foundation
import MapKit
import WarrenLogging
import Operations

private let locationMarkerReuseIdentifier = "location"
private let geoJSONSourceFileName = "countries.geo.json"

final class MapViewController: UIViewController, MKMapViewDelegate {
    private let logger = Logger(label: "MapViewController")
    private let animationQueue = AsyncOperationQueue.makeSerial()

    private let locationMarker = MKPointAnnotation()
    private var willChangeRegion = false
    private var regionDidChangeCompletion: (() -> Void)?
    private let mapView = MKMapView()
    private var isFirstLayoutPass = true
    private var center: CLLocationCoordinate2D?
    var alignmentView: UIView?

    // MARK: - View lifecycle

    override func viewDidLoad() {
        super.viewDidLoad()

        mapView.delegate = self
        mapView.register(
            MKAnnotationView.self,
            forAnnotationViewWithReuseIdentifier: locationMarkerReuseIdentifier
        )

        mapView.showsUserLocation = false
        mapView.isZoomEnabled = false
        mapView.isScrollEnabled = false
        mapView.isUserInteractionEnabled = false
        mapView.accessibilityElementsHidden = true

        // Use dark style for the map to dim the map grid
        mapView.overrideUserInterfaceStyle = .dark

        addTileOverlay()
        loadGeoJSONData()
        addMapView()
    }

    override func viewWillTransition(
        to size: CGSize,
        with coordinator: UIViewControllerTransitionCoordinator
    ) {
        super.viewWillTransition(to: size, with: coordinator)

        coordinator.animate(
            alongsideTransition: nil,
            completion: { context in
                self.recomputeVisibleRegion(animated: context.isAnimated)
            })
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()

        if isFirstLayoutPass {
            isFirstLayoutPass = false
            recomputeVisibleRegion(animated: false)
        }
    }

    // MARK: - Public

    func addLocationMarker(coordinate: CLLocationCoordinate2D) {
        locationMarker.coordinate = coordinate
        mapView.addAnnotation(locationMarker)
    }

    func removeLocationMarker() {
        mapView.removeAnnotation(locationMarker)
    }

    func setCenter(
        _ center: CLLocationCoordinate2D?,
        animated: Bool,
        completion: (() -> Void)? = nil
    ) {
        enqueueAnimation(cancelOtherAnimations: true) { finish in
            self.setCenterInternal(center, animated: animated) {
                finish()
                completion?()
            }
        }
    }

    // MARK: - MKMapViewDelegate

    func mapView(_ mapView: MKMapView, rendererFor overlay: MKOverlay) -> MKOverlayRenderer {
        if let polygon = overlay as? MKPolygon {
            let renderer = MKPolygonRenderer(polygon: polygon)
            renderer.fillColor = .Map.landColor
            renderer.strokeColor = .Map.oceanColor
            renderer.lineWidth = 1
            renderer.lineCap = .round
            renderer.lineJoin = .round
            return renderer
        }

        if let tileOverlay = overlay as? MKTileOverlay {
            return CustomOverlayRenderer(overlay: tileOverlay)
        }

        return MKOverlayRenderer()
    }

    func mapView(_ mapView: MKMapView, viewFor annotation: MKAnnotation) -> MKAnnotationView? {
        guard annotation === locationMarker else { return nil }

        let view = mapView.dequeueReusableAnnotationView(
            withIdentifier: locationMarkerReuseIdentifier,
            for: annotation
        )
        view.isDraggable = false
        view.canShowCallout = false
        view.image = UIImage(named: "LocationMarkerSecure")

        return view
    }

    func mapView(_ mapView: MKMapView, regionWillChangeAnimated animated: Bool) {
        willChangeRegion = true
    }

    func mapView(_ mapView: MKMapView, regionDidChangeAnimated animated: Bool) {
        willChangeRegion = false

        let handler = regionDidChangeCompletion
        regionDidChangeCompletion = nil
        handler?()
    }

    // MARK: - Private

    private func addMapView() {
        mapView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(mapView)

        NSLayoutConstraint.activate([
            mapView.topAnchor.constraint(equalTo: view.topAnchor),
            mapView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            mapView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            mapView.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
    }

    private func addTileOverlay() {
        let tileOverlay = MKTileOverlay(urlTemplate: nil)
        tileOverlay.canReplaceMapContent = true

        mapView.addOverlay(tileOverlay, level: .aboveLabels)
    }

    private func loadGeoJSONData() {
        guard let fileURL = Bundle.main.url(forResource: geoJSONSourceFileName, withExtension: nil)
        else {
            logger.debug("Failed to locate \(geoJSONSourceFileName) in main bundle.")
            return
        }

        do {
            let data = try Data(contentsOf: fileURL)
            guard let features = try MKGeoJSONDecoder().decode(data) as? [MKGeoJSONFeature]
            else { return }

            var overlays = [MKOverlay]()

            for feature in features {
                for geometry in feature.geometry {
                    if let polygon = geometry as? MKPolygon {
                        if let interiorPolygons = polygon.interiorPolygons,
                            !interiorPolygons.isEmpty
                        {
                            overlays
                                .append(
                                    MKPolygon(
                                        points: polygon.points(),
                                        count: polygon.pointCount
                                    ))
                            overlays.append(contentsOf: interiorPolygons)
                        } else {
                            overlays.append(polygon)
                        }
                    }

                    if let multiPolygon = geometry as? MKMultiPolygon {
                        overlays.append(contentsOf: multiPolygon.polygons)
                    }
                }
            }

            mapView.addOverlays(overlays, level: .aboveLabels)
        } catch {
            logger.error(error: error, message: "Failed to load geojson.")
        }
    }

    private func setCenterInternal(
        _ center: CLLocationCoordinate2D?,
        animated: Bool,
        completion: (() -> Void)?
    ) {
        let region = makeRegion(center: center)

        self.center = center

        // Map view does not call delegate methods when attempting to set the same region.
        mapView.setRegion(region, animated: animated)

        if willChangeRegion {
            regionDidChangeCompletion = completion
        } else {
            completion?()
        }
    }

    private func recomputeVisibleRegion(animated: Bool) {
        enqueueAnimation(cancelOtherAnimations: false) { finish in
            self.setCenterInternal(self.center, animated: animated, completion: finish)
        }
    }

    private func enqueueAnimation(
        cancelOtherAnimations: Bool,
        block: @escaping (_ finish: @escaping () -> Void) -> Void
    ) {
        nonisolated(unsafe) let nonisolatedBlock = block
        let operation = AsyncBlockOperation(dispatchQueue: .main) { finish in
            nonisolatedBlock {
                finish(nil)
            }
        }

        if cancelOtherAnimations {
            animationQueue.cancelAllOperations()
        }

        animationQueue.addOperation(operation)
    }

    private func makeRegion(center: CLLocationCoordinate2D?) -> MKCoordinateRegion {
        guard let center else {
            return makeZoomedOutRegion()
        }

        let sourceRegion = makeZoomedInRegion(center: center)

        guard let alignmentView else {
            return sourceRegion
        }

        return makeRegion(from: sourceRegion, withCenterMatching: alignmentView)
    }

    private func makeZoomedInRegion(center: CLLocationCoordinate2D) -> MKCoordinateRegion {
        let span = MKCoordinateSpan(latitudeDelta: 30, longitudeDelta: 30)
        let region = MKCoordinateRegion(center: center, span: span)

        return mapView.regionThatFits(region)
    }

    private func makeZoomedOutRegion() -> MKCoordinateRegion {
        let coordinate = CLLocationCoordinate2D(latitude: 0, longitude: 0)
        let span = MKCoordinateSpan(latitudeDelta: 90, longitudeDelta: 90)
        let region = MKCoordinateRegion(center: coordinate, span: span)

        return mapView.regionThatFits(region)
    }

    private func makeRegion(
        from region: MKCoordinateRegion,
        withCenterMatching alignmentView: UIView
    ) -> MKCoordinateRegion {
        // Map view center lies within layout margins frame.
        let mapViewLayoutFrame = mapView.layoutMarginsGuide.layoutFrame

        guard mapViewLayoutFrame.width > 0, mapView.frame.width > 0,
            region.span.longitudeDelta > 0,
            mapView.region.span.longitudeDelta > 0
        else { return region }

        // MKMapView.convert(_:toRectTo:) returns CGRect scaled to the zoom level derived from
        // currently set region.
        // Calculate the ratio that we can use to translate the rect within its own coordinate
        // system before converting it into MKCoordinateRegion.
        let newZoomLevel = mapViewLayoutFrame.width / region.span.longitudeDelta
        let currentZoomLevel = mapViewLayoutFrame.width / mapView.region.span.longitudeDelta
        let zoomDelta = currentZoomLevel / newZoomLevel

        let alignmentViewRect = alignmentView.convert(alignmentView.bounds, to: mapView)
        let horizontalOffset = (mapViewLayoutFrame.midX - alignmentViewRect.midX) * zoomDelta
        let verticalOffset = (mapViewLayoutFrame.midY - alignmentViewRect.midY) * zoomDelta

        let regionRect = mapView.convert(region, toRectTo: mapView)
        let offsetRegionRect = regionRect.offsetBy(dx: horizontalOffset, dy: verticalOffset)
        let offsetRegion = mapView.convert(offsetRegionRect, toRegionFrom: mapView)

        if CLLocationCoordinate2DIsValid(offsetRegion.center) {
            return offsetRegion
        } else {
            return region
        }
    }
}

// MARK: - Warren scenery backdrop

/// Per-country scenery backdrop, the iOS port of the desktop Bula connect
/// screen: a full-bleed landscape for the exit country, the burrow foreground
/// and Bula the rabbit, who hides inside the burrow once the tunnel is up.
/// `TunnelViewController.usesSceneryBackdrop` switches between this and the
/// legacy `MapViewController`, which is kept intact so reverting is trivial.
final class SceneryViewController: UIViewController {
    // Only these exits have dedicated cityscape art; every other country falls
    // back to the generic plain. Keys are the normalized (lower-case, trimmed)
    // relay-list English country names, matching the desktop scenery lookup.
    private static let countryImages: [String: String] = [
        "finland": "SceneryFinland",
        "germany": "SceneryGermany",
        "netherlands": "SceneryNetherlands",
        "singapore": "ScenerySingapore",
    ]
    // The open plain, with the two cameras trained on it: home when no tunnel
    // carries the traffic, and the backdrop of any exit with no bespoke art.
    private static let plainImageName = "SceneryPlaine"

    // The burrow foreground and Bula are raised by a constant share of the
    // frame height so the burrow mouth clears the connection card. It is the
    // same in every phase on purpose: only the landscape crossfades, so the
    // foreground never jumps between modes. Desktop needs no lift at all, its
    // frame carries the canvas ratio so the master's own framing lands right;
    // the phone draws the foreground at canvas width (see the width-fit layout
    // below), far shorter than the bounds, so it would otherwise sink behind
    // the connection card.
    private static let foregroundLift: CGFloat = 0.07

    // Hiding slides Bula this share of the height down into the burrow.
    private static let bulaHideDrop: CGFloat = 0.03

    // Animation timings, matching the desktop CSS transitions (and the
    // Android SceneryBackdrop) so every platform breathes at the same pace.
    private static let crossfadeDuration: TimeInterval = 0.7
    private static let blurDuration: TimeInterval = 0.9
    private static let zoomDuration: TimeInterval = 6.0
    private static let bulaDuration: TimeInterval = 0.55

    private static let connectingZoom: CGFloat = 1.08
    private static let washAlpha: Float = 0.14
    private static let scrimStart: NSNumber = 0.66
    private static let scrimAlpha: CGFloat = 0.6

    // Gaussian radius in source-image pixels approximating the desktop
    // blur(14px): the 1706px-tall art shows at roughly half size on a phone.
    private static let blurRadius: Double = 30

    private struct Scenery: Equatable {
        let imageName: String
        // Whether Bula sits exposed on the grass (outside the burrow).
        let showsBula: Bool
        // Whether the landscape is blurred (the connecting animation).
        let blurred: Bool
    }

    // Landscape layers live in one container so the connecting zoom scales
    // them together (desktop wraps them in a single transformed Scene div).
    private let landscapeContainer = UIView()
    private let bottomExtensionView = SceneryViewController.makeFullBleedImageView()
    private let backLandscapeView = SceneryViewController.makeFullBleedImageView()
    private let frontLandscapeView = SceneryViewController.makeFullBleedImageView()
    private let blurredLandscapeView = SceneryViewController.makeFullBleedImageView()
    private let foregroundView = SceneryViewController.makeFullBleedImageView()
    private let bulaView = SceneryViewController.makeFullBleedImageView()

    // A faint accent tint reinforces the phase (terracotta exposed / orange
    // connecting / olive protected) without washing out the artwork.
    private let accentWashLayer = CAGradientLayer()

    // One continuous bottom scrim to the very screen edge (desktop
    // AppMainFooter): grounds the card and footer, and swallows the lifted
    // foreground's bottom edge so no seam bands show.
    private let bottomScrimLayer = CAGradientLayer()

    private var currentImageName: String?
    private var bulaVisible = true
    private var isBlurred = false

    // Blurring the full-resolution art costs tens of milliseconds; cache the
    // result per landscape so each country pays it once per process.
    private static var blurredImageCache = [String: UIImage]()

    // Blurred mirrored ground/water continuation per landscape (see
    // bottomExtensionImage), cached for the same reason.
    private static var bottomExtensionCache = [String: UIImage]()

    private static func makeFullBleedImageView() -> UIImageView {
        let imageView = UIImageView()
        imageView.contentMode = .scaleAspectFill
        imageView.clipsToBounds = false
        return imageView
    }

    override func viewDidLoad() {
        super.viewDidLoad()

        // Sky tone before the art paints (desktop backdrop fallback): a
        // missing scenery asset must degrade gracefully, never to a blue map.
        view.backgroundColor = UIColor(red: 120 / 255, green: 170 / 255, blue: 210 / 255, alpha: 1)
        view.clipsToBounds = true
        view.isUserInteractionEnabled = false
        view.accessibilityElementsHidden = true

        blurredLandscapeView.alpha = 0

        // The bottom extension is stretched haze: any sampling artifact
        // would show as banding at full sharpness, so it scales to fill.
        bottomExtensionView.contentMode = .scaleToFill

        [bottomExtensionView, backLandscapeView, frontLandscapeView, blurredLandscapeView].forEach {
            landscapeContainer.addSubview($0)
        }
        [landscapeContainer, foregroundView, bulaView].forEach {
            view.addSubview($0)
        }

        accentWashLayer.locations = [0, 0.22, 0.78, 1]
        accentWashLayer.opacity = Self.washAlpha
        accentWashLayer.compositingFilter = "softLightBlendMode"
        view.layer.addSublayer(accentWashLayer)

        bottomScrimLayer.colors = [
            UIColor.clear.cgColor,
            UIColor.clear.cgColor,
            UIColor.black.withAlphaComponent(Self.scrimAlpha).cgColor,
        ]
        bottomScrimLayer.locations = [0, Self.scrimStart, 1]
        view.layer.addSublayer(bottomScrimLayer)

        foregroundView.image = UIImage(named: "SceneryTerrier")
        bulaView.image = UIImage(named: "SceneryBula")
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()

        let bounds = view.bounds

        // Center + bounds rather than frame: the container carries the
        // connecting-zoom transform and setting frame under a non-identity
        // transform is undefined.
        landscapeContainer.bounds = bounds
        landscapeContainer.center = CGPoint(x: bounds.midX, y: bounds.midY)

        // The landscape is width-fit and top-anchored, NOT center-cropped:
        // a tall phone would otherwise cut ~30% of the art's width and
        // lose edge elements like the country flag. The gap this opens
        // below the art is filled by the blurred ground/water
        // continuation, tucked under the bottom scrim and the card.
        let layout = Self.landscapeLayout(
            imageSize: (backLandscapeView.image ?? frontLandscapeView.image)?.size,
            in: bounds
        )
        [backLandscapeView, frontLandscapeView, blurredLandscapeView].forEach {
            $0.frame = layout.landscape
        }
        bottomExtensionView.frame = layout.bottomExtension
        bottomExtensionView.isHidden = layout.bottomExtension.height <= 0

        // The foreground layers are NOT height-cropped like the landscape:
        // width-fit keeps the whole painted canvas in frame (the burrow never
        // crops off screen and Bula stays at his painted size instead of the
        // tall-screen zoom), anchored at the bottom edge and lifted from
        // there. Placed via bounds + center because the lift transform is
        // already applied (setting frame under a transform is undefined).
        place(foregroundView, at: Self.widthFitBottomFrame(for: foregroundView.image, in: bounds))
        place(bulaView, at: Self.widthFitBottomFrame(for: bulaView.image, in: bounds))
        foregroundView.transform = CGAffineTransform(
            translationX: 0, y: -bounds.height * Self.foregroundLift)
        bulaView.transform = bulaTransform(visible: bulaVisible)

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        accentWashLayer.frame = bounds
        bottomScrimLayer.frame = bounds
        CATransaction.commit()
    }

    private static func widthFitBottomFrame(for image: UIImage?, in bounds: CGRect) -> CGRect {
        guard let size = image?.size, size.width > 0 else { return bounds }
        let height = bounds.width * size.height / size.width
        return CGRect(x: 0, y: bounds.height - height, width: bounds.width, height: height)
    }

    struct SceneryLayout: Equatable {
        let landscape: CGRect
        // The strip left below the art on tall screens; zero-height when
        // the art already covers the screen (wide screens). Overlaps the
        // art bottom edge by 1pt so no hairline gap can show.
        let bottomExtension: CGRect
    }

    // Pure layout math, separated from UIKit so it is directly testable:
    // the landscape shows its FULL painted width anchored to the TOP
    // edge (the real painted sky stays sharp behind the header, and edge
    // elements like the country flag stay in frame), and whatever screen
    // remains below it belongs to the bottom extension, which hides
    // under the bottom scrim and the connection card.
    static func landscapeLayout(imageSize: CGSize?, in bounds: CGRect) -> SceneryLayout {
        guard let size = imageSize, size.width > 0 else {
            return SceneryLayout(landscape: bounds, bottomExtension: .zero)
        }
        let height = bounds.width * size.height / size.width
        let landscape = CGRect(x: 0, y: 0, width: bounds.width, height: height)
        let bottomExtension =
            height < bounds.height
            ? CGRect(
                x: 0, y: height - 1, width: bounds.width, height: bounds.height - height + 1)
            : .zero
        return SceneryLayout(landscape: landscape, bottomExtension: bottomExtension)
    }

    private func place(_ view: UIView, at rect: CGRect) {
        view.bounds = CGRect(origin: .zero, size: rect.size)
        view.center = CGPoint(x: rect.midX, y: rect.midY)
    }

    func update(phase: ConnectionPhase, exitCountry: String?, animated: Bool) {
        loadViewIfNeeded()

        let scenery = Self.resolveScenery(phase: phase, exitCountry: exitCountry)

        setLandscape(imageName: scenery.imageName, animated: animated)
        setBlurred(scenery.blurred, animated: animated)
        setBulaVisible(scenery.showsBula, animated: animated)
        setAccent(color: phase.accentColor, animated: animated)
    }

    // The scenery is driven purely by the visual phase plus, when connecting
    // or protected, the exit country. Without a tunnel the backdrop is the
    // watched plain, so an unprotected screen shows what unprotected means,
    // and the country art is reserved for the states where traffic really goes
    // there.
    private static func resolveScenery(phase: ConnectionPhase, exitCountry: String?) -> Scenery {
        switch phase {
        case .exposed:
            return Scenery(imageName: plainImageName, showsBula: true, blurred: false)
        case .connecting:
            // Background swaps to the target country and blurs; the rabbit is
            // left outside until the tunnel is actually up.
            return Scenery(imageName: countryImage(for: exitCountry), showsBula: true, blurred: true)
        case .protected:
            return Scenery(imageName: countryImage(for: exitCountry), showsBula: false, blurred: false)
        case .interrupted:
            // Nominally-up tunnel with nothing flowing: same visual language
            // as the connecting transition (blurred city) so the scene reads
            // "not settled", with the rabbit still tucked in (fail-closed).
            return Scenery(imageName: countryImage(for: exitCountry), showsBula: false, blurred: true)
        case .blocked:
            // Kill switch: the rabbit is tucked in, so the watched world
            // outside is only seen through the blur, never sharp like exposed.
            return Scenery(imageName: plainImageName, showsBula: false, blurred: true)
        }
    }

    private static func countryImage(for country: String?) -> String {
        let key = (country ?? "").trimmingCharacters(in: .whitespaces).lowercased()
        return countryImages[key] ?? plainImageName
    }

    private func setLandscape(imageName: String, animated: Bool) {
        guard imageName != currentImageName else { return }
        currentImageName = imageName

        let image = UIImage(named: imageName)
        refreshBlurredLandscape(animated: animated)
        refreshBottomExtension()
        // Frames derive from the image size (width-fit layout); the first
        // image assignment must trigger a layout pass.
        view.setNeedsLayout()
        guard animated, backLandscapeView.image != nil else {
            backLandscapeView.image = image
            frontLandscapeView.image = image
            return
        }

        // Cross-fade: the incoming landscape fades in over the previous one,
        // which stays opaque underneath, then becomes the new base layer.
        frontLandscapeView.layer.removeAllAnimations()
        frontLandscapeView.image = image
        frontLandscapeView.alpha = 0
        UIView.animate(withDuration: Self.crossfadeDuration, delay: 0, options: [.curveEaseInOut]) {
            self.frontLandscapeView.alpha = 1
        } completion: { finished in
            if finished {
                self.backLandscapeView.image = image
            }
        }
    }

    // "The destination is not in focus yet": while connecting the landscape
    // gaussian-blurs (a pre-rendered variant fading in, the same 14pt look as
    // desktop/Android) and slowly zooms. The foreground layers stay sharp on
    // purpose: only the landscape tells the transition story.
    private func setBlurred(_ blurred: Bool, animated: Bool) {
        guard blurred != isBlurred else { return }
        isBlurred = blurred

        if blurred {
            refreshBlurredLandscape(animated: false)
        }
        UIView.animate(withDuration: animated ? Self.blurDuration : 0) {
            self.blurredLandscapeView.alpha = blurred ? 1 : 0
        }
        UIView.animate(
            withDuration: animated ? Self.zoomDuration : 0, delay: 0, options: [.curveEaseOut]
        ) {
            self.landscapeContainer.transform =
                blurred
                ? CGAffineTransform(scaleX: Self.connectingZoom, y: Self.connectingZoom)
                : .identity
        }
    }

    // Keeps the blurred overlay in sync with the current landscape. The
    // gaussian render happens off the main thread on first use per image
    // (cached afterwards) so the connect tap never stutters.
    private func refreshBlurredLandscape(animated: Bool) {
        guard let imageName = currentImageName else { return }

        if let cached = Self.blurredImageCache[imageName] {
            applyBlurredImage(cached, animated: animated)
            return
        }
        guard let source = UIImage(named: imageName) else { return }
        DispatchQueue.global(qos: .userInitiated).async {
            guard let blurred = Self.gaussianBlurred(source, radius: Self.blurRadius) else { return }
            DispatchQueue.main.async {
                Self.blurredImageCache[imageName] = blurred
                // Only apply if this landscape is still the visible one.
                if self.currentImageName == imageName {
                    self.applyBlurredImage(blurred, animated: true)
                }
            }
        }
    }

    // Keeps the ground/water continuation in sync with the current
    // landscape, generated off the main thread on first use per image
    // (cached after).
    private func refreshBottomExtension() {
        guard let imageName = currentImageName else { return }

        if let cached = Self.bottomExtensionCache[imageName] {
            applyBottomExtension(cached)
            return
        }
        guard let source = UIImage(named: imageName) else { return }
        DispatchQueue.global(qos: .userInitiated).async {
            guard let continuation = Self.bottomExtensionImage(from: source) else { return }
            DispatchQueue.main.async {
                Self.bottomExtensionCache[imageName] = continuation
                if self.currentImageName == imageName {
                    self.applyBottomExtension(continuation)
                }
            }
        }
    }

    private func applyBottomExtension(_ image: UIImage) {
        guard bottomExtensionView.image !== image else { return }
        UIView.transition(
            with: bottomExtensionView,
            duration: bottomExtensionView.image == nil ? 0 : Self.crossfadeDuration,
            options: [.transitionCrossDissolve]
        ) {
            self.bottomExtensionView.image = image
        }
    }

    // Continuation below the painted canvas: the bottom strip of the
    // art, mirrored vertically, sharp at the seam and progressively
    // blurring away from it. The unblurred seam row is pixel-identical
    // to the art's bottom edge, so no line can show; the deep-blur zone
    // sits behind the bottom scrim and the connection card anyway.
    private static func bottomExtensionImage(from image: UIImage) -> UIImage? {
        guard let cgImage = image.cgImage else { return nil }
        let stripHeight = max(1, Int(CGFloat(cgImage.height) * 0.45))
        guard
            let stripCG = cgImage.cropping(
                to: CGRect(
                    x: 0,
                    y: cgImage.height - stripHeight,
                    width: cgImage.width,
                    height: stripHeight
                ))
        else { return nil }

        let strip = CIImage(cgImage: stripCG)
        let extent = strip.extent
        let blurred =
            strip
            .clampedToExtent()
            .applyingGaussianBlur(sigma: 40)
            .cropped(to: extent)

        // The art's bottom edge row renders at extent.minY in Core Image
        // coordinates; displayed .downMirrored it becomes the top of the
        // extension, i.e. the seam. The mask keeps that side sharp
        // (black = background) and reaches full blur (white = input) a
        // third of the strip away.
        guard let gradientFilter = CIFilter(name: "CISmoothLinearGradient") else { return nil }
        gradientFilter.setValue(CIVector(x: 0, y: extent.minY), forKey: "inputPoint0")
        gradientFilter.setValue(CIColor.black, forKey: "inputColor0")
        gradientFilter.setValue(
            CIVector(x: 0, y: extent.minY + extent.height * 0.35), forKey: "inputPoint1")
        gradientFilter.setValue(CIColor.white, forKey: "inputColor1")
        guard let mask = gradientFilter.outputImage?.cropped(to: extent),
            let blendFilter = CIFilter(name: "CIBlendWithMask")
        else { return nil }
        blendFilter.setValue(blurred, forKey: kCIInputImageKey)
        blendFilter.setValue(strip, forKey: kCIInputBackgroundImageKey)
        blendFilter.setValue(mask, forKey: kCIInputMaskImageKey)

        guard let output = blendFilter.outputImage,
            let outCG = CIContext(options: nil).createCGImage(output, from: extent)
        else { return nil }
        return UIImage(cgImage: outCG, scale: image.scale, orientation: .downMirrored)
    }

    private func applyBlurredImage(_ image: UIImage, animated: Bool) {
        guard blurredLandscapeView.image !== image else { return }
        UIView.transition(
            with: blurredLandscapeView,
            duration: animated && blurredLandscapeView.alpha > 0 ? Self.crossfadeDuration : 0,
            options: [.transitionCrossDissolve]
        ) {
            self.blurredLandscapeView.image = image
        }
    }

    private static func gaussianBlurred(_ image: UIImage, radius: Double) -> UIImage? {
        guard let ciImage = CIImage(image: image) else { return nil }
        // Clamp the edges before blurring so the border does not fade to
        // transparent, then crop back to the original extent.
        let blurred =
            ciImage
            .clampedToExtent()
            .applyingGaussianBlur(sigma: radius)
            .cropped(to: ciImage.extent)
        let context = CIContext(options: nil)
        guard let cgImage = context.createCGImage(blurred, from: blurred.extent) else { return nil }
        return UIImage(cgImage: cgImage, scale: image.scale, orientation: image.imageOrientation)
    }

    // Bula rides the same lift as the burrow he sits on; hiding slides him
    // slightly down into the burrow while fading out.
    private func bulaTransform(visible: Bool) -> CGAffineTransform {
        let lift = -view.bounds.height * Self.foregroundLift
        let hideOffset = view.bounds.height * Self.bulaHideDrop
        return CGAffineTransform(translationX: 0, y: visible ? lift : lift + hideOffset)
    }

    private func setBulaVisible(_ visible: Bool, animated: Bool) {
        guard visible != bulaVisible else { return }
        bulaVisible = visible

        UIView.animate(
            withDuration: animated ? Self.bulaDuration : 0, delay: 0, options: [.curveEaseInOut]
        ) {
            self.bulaView.alpha = visible ? 1 : 0
            self.bulaView.transform = self.bulaTransform(visible: visible)
        }
    }

    private func setAccent(color: UIColor, animated: Bool) {
        CATransaction.begin()
        CATransaction.setAnimationDuration(animated ? Self.crossfadeDuration : 0)
        accentWashLayer.colors = [
            color.cgColor,
            UIColor.clear.cgColor,
            UIColor.clear.cgColor,
            color.cgColor,
        ]
        CATransaction.commit()
    }
}
