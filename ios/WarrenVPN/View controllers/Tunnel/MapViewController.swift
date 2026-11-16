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

// MARK: - Warren scenery hero

/// The home hero: the scenery framed so that what matters in the current phase
/// stays clear of the floating header and of the fade into the panel (desktop
/// `CountryBackdrop`). The three registered layers (the landscape of the exit
/// country or the watched plain, the burrow, and Bula, who hides inside the
/// burrow once the tunnel is up) are drawn in one canvas rect that
/// `SceneryFraming` places.
final class SceneryViewController: UIViewController {
    // Only the countries in the table have dedicated cityscape art; every other
    // country falls back to the generic plain, the open plain with the two
    // cameras trained on it: home when no tunnel carries the traffic, and the
    // backdrop of any exit with no bespoke art.
    // BEGIN GENERATED scenery table: scripts/gen-scenery-tables.mjs from scenery.json, do not edit.
    // Keyed by the lower-case ISO code and the lower-case English relay-list name.
    private static let countryImages: [String: String] = [
        "de": "SceneryGermany",
        "germany": "SceneryGermany",
        "fi": "SceneryFinland",
        "finland": "SceneryFinland",
        "fr": "SceneryFrance",
        "france": "SceneryFrance",
        "nl": "SceneryNetherlands",
        "netherlands": "SceneryNetherlands",
        "ro": "SceneryRomania",
        "romania": "SceneryRomania",
        "sg": "ScenerySingapore",
        "singapore": "ScenerySingapore",
    ]
    private static let plainImageName = "SceneryPlaine"
    private static let burrowImageName = "SceneryTerrier"
    private static let bulaImageName = "SceneryBula"

    // How each landscape is framed in the home hero: its skyline row and the ARGB of its top rows.
    static let landscapeFraming: [String: (skylineTopRow: Int, topFill: UInt32)] = [
        "SceneryPlaine": (skylineTopRow: 313, topFill: 0xFFD5DEE7),
        "SceneryGermany": (skylineTopRow: 140, topFill: 0xFFF4F3F2),
        "SceneryFinland": (skylineTopRow: 235, topFill: 0xFFF9F5F0),
        "SceneryFrance": (skylineTopRow: 160, topFill: 0xFFF1F2F3),
        "SceneryNetherlands": (skylineTopRow: 100, topFill: 0xFFEFF2F3),
        "SceneryRomania": (skylineTopRow: 138, topFill: 0xFFF8F7F6),
        "ScenerySingapore": (skylineTopRow: 350, topFill: 0xFFFCF7F1),
    ]

    // Which landscape a phase shows (the exit country, or the plain), and the other two layers.
    private static func sceneryRow(
        _ phase: ConnectionPhase
    ) -> (countryLandscape: Bool, showsBula: Bool, blurred: Bool, skyFraming: Bool) {
        switch phase {
        case .exposed:
            return (countryLandscape: false, showsBula: true, blurred: false, skyFraming: false)
        case .connecting:
            return (countryLandscape: true, showsBula: true, blurred: true, skyFraming: false)
        case .protected:
            return (countryLandscape: true, showsBula: false, blurred: false, skyFraming: true)
        case .interrupted:
            return (countryLandscape: true, showsBula: false, blurred: true, skyFraming: true)
        case .blocked:
            return (countryLandscape: false, showsBula: false, blurred: true, skyFraming: false)
        }
    }
    // END GENERATED scenery table
    // Hiding slides Bula this share of the canvas height down into the burrow.
    private static let bulaHideDrop: CGFloat = 0.03

    // Animation timings, matching the desktop CSS transitions (and the
    // Android SceneryBackdrop) so every platform breathes at the same pace.
    private static let crossfadeDuration = TimeInterval(HomeMetrics.Hero.crossfadeMs / 1000)
    private static let frameDuration = TimeInterval(HomeMetrics.Hero.frameMs / 1000)
    private static let bulaDuration = TimeInterval(HomeMetrics.Hero.bulaMs / 1000)
    private static let blurDuration: TimeInterval = 0.9
    private static let zoomDuration: TimeInterval = 6.0
    private static let minimumBlurWidth: CGFloat = 64
    private static let minimumBandFeather: CGFloat = 40

    /// The status bar the floating header sits under. The art is clear of the
    /// header below `HomeMetrics.clearTop(safeTop:)`.
    var safeTop: CGFloat = 0 {
        didSet {
            guard safeTop != oldValue else { return }
            view.setNeedsLayout()
        }
    }

    /// The hero height the art is framed for, the one the hero is heading to rather than the one
    /// an animated resize is passing through, so the art never shrinks to fit a hero halfway open.
    /// Nil frames for the view's own height.
    private var targetHeight: CGFloat?
    /// The hero's height with the details closed, whose scale a compact frame keeps.
    private var openHeight: CGFloat?
    private var compact = false

    private var phase: ConnectionPhase = .exposed
    private var currentImageName: String?
    private var bulaVisible = true
    private var isBlurred = false
    /// The drawn canvas width the visible blurred render was made for, so a
    /// reframe re-renders once rather than on every layout pass.
    private var lastBlurredWidth: CGFloat = 0
    private var frame: SceneryFraming.Frame?

    // Behind a canvas narrower than the hero: the same landscape at cover scale,
    // blurred, so the side margins carry the scene on instead of a flat colour.
    private let backdropView = SceneryViewController.makeLayerView()
    private var backdropRect: CGRect?
    /// The landscape and drawn width the visible backdrop was blurred for.
    private var backdropKey: String?
    // Over the backdrop, the band above a dropped canvas keeps the landscape's
    // top fill across the hero, its lower edge feathered into the blur.
    private let headroomView = UIView()
    private let headroomMask = GradientView()

    // The registered canvas: every layer fills it exactly, so moving or scaling
    // it moves them together.
    private let canvasView = SceneryCanvasView()
    // The canvas edges facing a headroom strip or a side margin, feathered into
    // the top fill behind them.
    private let featherView = UIImageView()
    // Landscape layers live in one container so the connecting zoom scales
    // them together (desktop wraps them in a single transformed Scene div).
    private let landscapeContainer = UIView()
    private let backLandscapeView = SceneryViewController.makeLayerView()
    private let frontLandscapeView = SceneryViewController.makeLayerView()
    private let blurredLandscapeView = SceneryViewController.makeLayerView()
    // The connecting dim covers the landscape only, as on desktop where the
    // filter sits on the scene wrapper: the burrow and Bula stay bright.
    private let connectingDimView = UIView()
    private let burrowView = SceneryViewController.makeLayerView()
    private let bulaView = SceneryViewController.makeLayerView()
    // The page colour over the top of the sky, so the toolbar reads on the
    // brightest plates, and a paper glow behind the inked logo.
    private let scrimView = GradientView()
    private let glowView = GradientView()
    // Fades the hero into the page along the mockup's curve: the status block
    // rides up over the last rows, so the art has no edge.
    private let fadeView = GradientView()
    // Beside the panel of a split home, the hero's edge facing it fades into the page too.
    private let sideFadeView = GradientView()

    /// The hero sits beside the panel rather than over it.
    var fadesPanelEdge = false {
        didSet {
            guard fadesPanelEdge != oldValue else { return }
            view.setNeedsLayout()
        }
    }

    // Blurring the full-resolution art costs tens of milliseconds; cache the
    // result per landscape and width. NSCache rather than a dictionary: these
    // are 7 MB entries and the process must be able to give them back under
    // pressure.
    private static let blurredImageCache = NSCache<NSString, UIImage>()

    // One Core Image context for every render here: building one per call
    // rebuilds its Metal pipeline each time.
    private static let ciContext = CIContext(options: nil)

    private static func makeLayerView() -> UIImageView {
        let imageView = UIImageView()
        imageView.contentMode = .scaleToFill
        return imageView
    }

    private static var animates: Bool { !UIAccessibility.isReduceMotionEnabled }

    override func viewDidLoad() {
        super.viewDidLoad()

        view.clipsToBounds = true
        view.isUserInteractionEnabled = false
        view.accessibilityElementsHidden = true
        view.backgroundColor = Self.topFill(of: Self.plainImageName)

        blurredLandscapeView.alpha = 0
        connectingDimView.backgroundColor = .black
        connectingDimView.alpha = 0

        [backLandscapeView, frontLandscapeView, blurredLandscapeView, connectingDimView].forEach {
            landscapeContainer.addSubview($0)
        }
        canvasView.landscapeContainer = landscapeContainer
        canvasView.layers = [burrowView, bulaView]
        [landscapeContainer, burrowView, bulaView].forEach { canvasView.addSubview($0) }
        featherView.contentMode = .scaleToFill
        canvasView.featherView = featherView

        backdropView.isHidden = true
        headroomView.isHidden = true
        headroomMask.gradientLayer.startPoint = CGPoint(x: 0.5, y: 0)
        headroomMask.gradientLayer.endPoint = CGPoint(x: 0.5, y: 1)
        headroomView.mask = headroomMask

        [backdropView, headroomView, canvasView, scrimView, glowView].forEach { view.addSubview($0) }

        fadeView.gradientLayer.colors = SceneryFraming.maskStops.map {
            UIColor.black.withAlphaComponent($0.alpha).cgColor
        }
        fadeView.gradientLayer.locations = SceneryFraming.maskStops.map { NSNumber(value: Double($0.at)) }
        view.mask = fadeView

        glowView.gradientLayer.type = .radial
        glowView.gradientLayer.startPoint = CGPoint(x: 0, y: 0)
        glowView.gradientLayer.endPoint = CGPoint(x: 1, y: 1)
        glowView.gradientLayer.locations = [0, 0.45, 0.75]

        burrowView.image = UIImage(named: Self.burrowImageName)
        bulaView.image = UIImage(named: Self.bulaImageName)

        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (self: Self, _) in
            self.applyThemeColors()
        }
        applyThemeColors()

        NotificationCenter.default.addObserver(
            self,
            selector: #selector(dropDerivedImages),
            name: UIApplication.didReceiveMemoryWarningNotification,
            object: nil
        )
    }

    @objc private func dropDerivedImages() {
        Self.blurredImageCache.removeAllObjects()
    }

    private func applyThemeColors() {
        let palette = ConnectSurfacePalette(theme: traitCollection.userInterfaceStyle == .light ? .light : .dark)
        scrimView.gradientLayer.colors = [
            palette.uiColor(.homeScrim).cgColor,
            palette.uiColor(.homeScrim).withAlphaComponent(0).cgColor,
        ]
        let glow = palette.uiColor(.homeLogoGlow)
        var alpha: CGFloat = 0
        glow.getRed(nil, green: nil, blue: nil, alpha: &alpha)
        glowView.gradientLayer.colors = [
            glow.cgColor,
            glow.withAlphaComponent(alpha * 0.43).cgColor,
            glow.withAlphaComponent(0).cgColor,
        ]
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()

        let bounds = view.bounds
        guard bounds.width > 0, bounds.height > 0 else { return }

        let imageName = currentImageName ?? Self.plainImageName
        let width = bounds.width
        let height = targetHeight ?? bounds.height
        let clearTop = HomeMetrics.clearTop(safeTop: safeTop)
        let span = SceneryFraming.span(
            skyFraming: Self.sceneryRow(phase).skyFraming,
            skylineTopRow: Self.skylineTopRow(of: imageName)
        )
        let frame = SceneryFraming.frame(
            width: width,
            height: height,
            clearTop: clearTop,
            fade: SceneryFraming.fade(forHeroHeight: height),
            span: span,
            compact: compact,
            fullHeight: openHeight
        )
        self.frame = frame
        canvasView.frame = frame.rect
        canvasView.setFeather(top: frame.headroom > 0.5, sides: frame.left > 0.5)
        bulaView.transform = bulaTransform(visible: bulaVisible)

        let backdrop = SceneryFraming.marginBackdrop(width: width, height: height, frame: frame, span: span)
        backdropRect = backdrop
        backdropView.isHidden = backdrop == nil
        if let backdrop {
            backdropView.frame = backdrop
            refreshBackdrop()
        }
        // Without a backdrop the hero's own top fill already shows above the canvas.
        headroomView.isHidden = backdrop == nil || frame.headroom <= 0.5
        if !headroomView.isHidden {
            // The canvas's own top edge already fades into this band; the band fades into the
            // blur under it over at least the same rows, and never over a line too thin to read
            // as a fade when the canvas is drawn small.
            let feather = max(SceneryFraming.featherRows * frame.scale, Self.minimumBandFeather)
            headroomView.frame = CGRect(x: 0, y: 0, width: width, height: frame.headroom + feather)
            headroomMask.frame = headroomView.bounds
            headroomMask.gradientLayer.colors = [
                UIColor.black.cgColor, UIColor.black.cgColor, UIColor.black.withAlphaComponent(0).cgColor,
            ]
            headroomMask.gradientLayer.locations = [
                0, NSNumber(value: Double(frame.headroom / headroomView.bounds.height)), 1,
            ]
        }

        // The blur radius is a fraction of the drawn width, so a reframe or a
        // resize needs a fresh render at the new width.
        if isBlurred, abs(frame.width - lastBlurredWidth) > 1 {
            refreshBlurredLandscape(animated: false)
        }

        fadeView.frame = bounds
        if fadesPanelEdge {
            let towardsLeft = view.effectiveUserInterfaceLayoutDirection == .rightToLeft
            sideFadeView.frame = fadeView.bounds
            sideFadeView.gradientLayer.startPoint = CGPoint(x: towardsLeft ? 1 : 0, y: 0.5)
            sideFadeView.gradientLayer.endPoint = CGPoint(x: towardsLeft ? 0 : 1, y: 0.5)
            sideFadeView.gradientLayer.colors = [
                UIColor.black.cgColor, UIColor.black.cgColor, UIColor.black.withAlphaComponent(0).cgColor,
            ]
            let edge = HomeMetrics.Panel.paddingHorizontal / max(bounds.width, 1)
            sideFadeView.gradientLayer.locations = [0, NSNumber(value: Double(1 - edge)), 1]
            fadeView.mask = sideFadeView
        } else {
            fadeView.mask = nil
        }
        scrimView.frame = bounds
        scrimView.gradientLayer.locations = [0, NSNumber(value: Double(HomeMetrics.Hero.scrimStop))]
        // Anchored at the corner like desktop's, and stretched by the status
        // bar so the halo still reaches the logo under it.
        glowView.frame = CGRect(
            x: 0, y: 0,
            width: HomeMetrics.Hero.glowWidth,
            height: HomeMetrics.Hero.glowHeight + safeTop
        )
    }

    /// The canvas rect in the hero, once laid out. For the screenshot QA.
    var canvasFrame: SceneryFraming.Frame? { frame }

    /// Where the blurred backdrop sits behind a narrowed canvas, nil when the canvas spans the hero.
    var marginBackdropFrame: CGRect? { backdropView.isHidden ? nil : backdropRect }

    /// Frames the art for a hero of `height` (`openHeight` with the details closed), the compact
    /// way when `compact`. Animated on the hero's own resize curve, so the art and the hero edge
    /// move as one.
    func setHero(height: CGFloat, openHeight: CGFloat, compact: Bool, animated: Bool) {
        loadViewIfNeeded()
        guard height != targetHeight || openHeight != self.openHeight || compact != self.compact else { return }
        let animated = animated && Self.animates && view.window != nil && targetHeight != nil
        targetHeight = height
        self.openHeight = openHeight
        self.compact = compact
        view.setNeedsLayout()
        guard animated else {
            view.layoutIfNeeded()
            return
        }
        let (first, second) = SceneryFraming.compactTransitionCurve
        let animator = UIViewPropertyAnimator(
            duration: TimeInterval(SceneryFraming.compactTransitionMs / 1000),
            controlPoint1: first,
            controlPoint2: second
        ) {
            self.view.layoutIfNeeded()
        }
        animator.startAnimation()
    }

    func update(phase: ConnectionPhase, exitCountry: String?, animated: Bool) {
        loadViewIfNeeded()
        let animated = animated && Self.animates && view.window != nil

        let row = Self.sceneryRow(phase)
        let imageName = row.countryLandscape ? Self.countryImage(for: exitCountry) : Self.plainImageName
        let reframes = phase != self.phase || imageName != currentImageName
        self.phase = phase

        setLandscape(imageName: imageName, animated: animated)
        setBlurred(row.blurred, animated: animated)
        setBulaVisible(row.showsBula, animated: animated)

        guard reframes else { return }
        view.setNeedsLayout()
        if animated {
            UIView.animate(withDuration: Self.frameDuration, delay: 0, options: [.curveEaseInOut]) {
                self.view.layoutIfNeeded()
            }
        } else {
            view.layoutIfNeeded()
        }
    }

    private func setLandscape(imageName: String, animated: Bool) {
        guard imageName != currentImageName else { return }
        currentImageName = imageName

        UIView.animate(withDuration: animated ? Self.crossfadeDuration : 0) {
            self.view.backgroundColor = Self.topFill(of: imageName)
            self.headroomView.backgroundColor = Self.topFill(of: imageName)
        }
        refreshBackdrop()

        let image = UIImage(named: imageName)
        refreshBlurredLandscape(animated: animated)
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
    // gaussian-blurs (a pre-rendered variant fading in, the same look as
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
            self.connectingDimView.alpha = blurred ? SceneryFraming.connectingDim : 0
        }
        UIView.animate(
            withDuration: animated ? Self.zoomDuration : 0, delay: 0, options: [.curveEaseOut]
        ) {
            self.landscapeContainer.transform =
                blurred
                ? CGAffineTransform(scaleX: SceneryFraming.connectingZoom, y: SceneryFraming.connectingZoom)
                : .identity
        }
    }

    // Keeps the blurred overlay in sync with the current landscape. The
    // gaussian render happens off the main thread on first use per image and
    // width (cached afterwards) so the connect tap never stutters.
    private func refreshBlurredLandscape(animated: Bool) {
        guard let imageName = currentImageName else { return }

        let width = frame?.width ?? view.bounds.width
        // A canvas framed for a hero not laid out yet is a sliver, whose blur radius in canvas
        // pixels would ask Core Image for gigabytes; the real layout renders it.
        guard width >= Self.minimumBlurWidth else { return }
        lastBlurredWidth = width
        let radius = Double(SceneryFraming.blurRadius(forDrawnWidth: width))
        // Carried across the hop as a String: NSString is a class, so it is not
        // Sendable and crossing with it is a data race under Swift 6.
        let key = "\(imageName)@\(Int(width.rounded()))"

        if let cached = Self.blurredImageCache.object(forKey: key as NSString) {
            applyBlurredImage(cached, animated: animated)
            return
        }
        guard let source = UIImage(named: imageName) else { return }
        DispatchQueue.global(qos: .userInitiated).async {
            guard let blurred = Self.gaussianBlurred(source, radius: radius) else { return }
            DispatchQueue.main.async {
                Self.blurredImageCache.setObject(blurred, forKey: key as NSString)
                // Only apply if this landscape is still the visible one.
                if self.currentImageName == imageName {
                    self.applyBlurredImage(blurred, animated: true)
                }
            }
        }
    }

    // The margin backdrop: the landscape blurred by `marginBlur` at the size it is drawn, rendered
    // off the main thread once per landscape and width, like the connecting blur.
    private func refreshBackdrop() {
        guard let imageName = currentImageName, let rect = backdropRect, rect.width >= Self.minimumBlurWidth else {
            return
        }
        let key = "\(imageName)#margin@\(Int(rect.width.rounded()))"
        guard key != backdropKey else { return }
        backdropKey = key
        if let cached = Self.blurredImageCache.object(forKey: key as NSString) {
            setBackdrop(cached)
            return
        }
        guard let source = UIImage(named: imageName) else { return }
        let radius = Double(SceneryFraming.marginBlur * SceneryFraming.canvasWidth / rect.width)
        DispatchQueue.global(qos: .userInitiated).async {
            // A wash this wide keeps nothing a full-size render would add, so it is blurred small.
            guard let blurred = Self.gaussianBlurred(source, radius: radius, downscale: 0.125) else { return }
            DispatchQueue.main.async {
                Self.blurredImageCache.setObject(blurred, forKey: key as NSString)
                if self.backdropKey == key {
                    self.setBackdrop(blurred)
                }
            }
        }
    }

    private func setBackdrop(_ image: UIImage) {
        UIView.transition(
            with: backdropView,
            duration: backdropView.image == nil || !Self.animates ? 0 : Self.crossfadeDuration,
            options: [.transitionCrossDissolve]
        ) {
            self.backdropView.image = image
        }
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

    private static func gaussianBlurred(_ image: UIImage, radius: Double, downscale: Double = 1) -> UIImage? {
        guard let source = CIImage(image: image) else { return nil }
        let ciImage =
            downscale < 1 ? source.transformed(by: CGAffineTransform(scaleX: downscale, y: downscale)) : source
        // Clamp the edges before blurring so the border does not fade to
        // transparent, then crop back to the original extent.
        let blurred =
            ciImage
            .clampedToExtent()
            .applyingGaussianBlur(sigma: radius * downscale)
            .cropped(to: ciImage.extent)
        guard let cgImage = ciContext.createCGImage(blurred, from: blurred.extent) else {
            return nil
        }
        return UIImage(cgImage: cgImage, scale: image.scale, orientation: image.imageOrientation)
    }

    // Bula is registered to the burrow he sits on; hiding slides him slightly
    // down into it while fading out.
    private func bulaTransform(visible: Bool) -> CGAffineTransform {
        let hideOffset = (frame?.height ?? view.bounds.height) * Self.bulaHideDrop
        return CGAffineTransform(translationX: 0, y: visible ? 0 : hideOffset)
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
}

// MARK: - Scenery lookups

extension SceneryViewController {
    private static func countryKey(_ country: String?) -> String {
        (country ?? "").trimmingCharacters(in: .whitespaces).lowercased()
    }

    private static func countryImage(for country: String?) -> String {
        countryImages[countryKey(country)] ?? plainImageName
    }

    /// The landscape a country is drawn with: its own art, or the plain. Keyed by
    /// the English relay-list name or the ISO code.
    static func landscapeImageName(forCountry country: String?) -> String {
        countryImage(for: country)
    }

    static func hasCountryArt(_ country: String?) -> Bool {
        countryImages[countryKey(country)] != nil
    }

    /// The landscape a phase shows: the exit country's while a tunnel carries
    /// the traffic there, the plain otherwise.
    static func phaseLandscape(_ phase: ConnectionPhase, country: String?) -> String {
        sceneryRow(phase).countryLandscape ? countryImage(for: country) : plainImageName
    }

    static func skylineTopRow(of imageName: String) -> Int {
        landscapeFraming[imageName]?.skylineTopRow
            ?? landscapeFraming[plainImageName]?.skylineTopRow
            ?? Int(SceneryFraming.groundSpan.top)
    }

    /// The colour of a landscape's top rows, which fills a headroom strip and
    /// the side margins of a narrowed canvas.
    static func topFill(of imageName: String) -> UIColor {
        let argb = landscapeFraming[imageName]?.topFill ?? landscapeFraming[plainImageName]?.topFill ?? 0xFFFF_FFFF
        return UIColor(
            red: CGFloat((argb >> 16) & 0xFF) / 255,
            green: CGFloat((argb >> 8) & 0xFF) / 255,
            blue: CGFloat(argb & 0xFF) / 255,
            alpha: CGFloat((argb >> 24) & 0xFF) / 255
        )
    }

    /// The canvas rows a phase keeps between the header and the fade.
    static func framingSpan(_ phase: ConnectionPhase, country: String?) -> SceneryFraming.Span {
        SceneryFraming.span(
            skyFraming: sceneryRow(phase).skyFraming,
            skylineTopRow: skylineTopRow(of: phaseLandscape(phase, country: country))
        )
    }
}

/// The registered canvas. Its layers fill it exactly; the landscape container is
/// placed by bounds and centre because it carries the connecting zoom, and
/// setting a frame under a transform is undefined.
private final class SceneryCanvasView: UIView {
    var landscapeContainer: UIView?
    var layers: [UIView] = []
    var featherView: UIImageView? {
        didSet { mask = featherView }
    }

    private var feather: (top: Bool, sides: Bool) = (false, false)
    private static var featherImages: [String: UIImage] = [:]

    func setFeather(top: Bool, sides: Bool) {
        guard feather != (top, sides) || featherView?.image == nil else { return }
        feather = (top, sides)
        featherView?.image = Self.featherImage(top: top, sides: sides)
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        if let landscapeContainer {
            landscapeContainer.bounds = bounds
            landscapeContainer.center = CGPoint(x: bounds.midX, y: bounds.midY)
            landscapeContainer.subviews.forEach { $0.frame = landscapeContainer.bounds }
        }
        for layer in layers {
            layer.bounds = bounds
            layer.center = CGPoint(x: bounds.midX, y: bounds.midY)
        }
        featherView?.frame = bounds
    }

    /// The canvas alpha: opaque, fading to nothing over `featherRows` at the
    /// top when a headroom strip sits above it and at both sides when it is
    /// narrower than the hero. Drawn small at the canvas ratio and stretched,
    /// since only the fractions matter.
    private static func featherImage(top: Bool, sides: Bool) -> UIImage {
        let key = "\(top)-\(sides)"
        if let cached = featherImages[key] { return cached }
        let size = CGSize(width: SceneryFraming.canvasWidth / 5, height: SceneryFraming.canvasHeight / 5)
        let topRows = SceneryFraming.featherRows / SceneryFraming.canvasHeight * size.height
        let sideColumns = SceneryFraming.featherRows / SceneryFraming.canvasWidth * size.width
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        let image = UIGraphicsImageRenderer(size: size, format: format).image { context in
            let cg = context.cgContext
            UIColor.black.setFill()
            cg.fill(CGRect(origin: .zero, size: size))
            let space = CGColorSpaceCreateDeviceGray()
            guard
                let ramp = CGGradient(
                    colorsSpace: space,
                    colors: [UIColor(white: 0, alpha: 1).cgColor, UIColor(white: 0, alpha: 0).cgColor] as CFArray,
                    locations: [0, 1]
                )
            else { return }
            cg.setBlendMode(.destinationOut)
            if top {
                cg.drawLinearGradient(
                    ramp, start: CGPoint(x: 0, y: 0), end: CGPoint(x: 0, y: topRows), options: [])
            }
            if sides {
                cg.drawLinearGradient(
                    ramp, start: CGPoint(x: 0, y: 0), end: CGPoint(x: sideColumns, y: 0), options: [])
                cg.drawLinearGradient(
                    ramp, start: CGPoint(x: size.width, y: 0), end: CGPoint(x: size.width - sideColumns, y: 0),
                    options: [])
            }
        }
        featherImages[key] = image
        return image
    }
}

/// A view whose layer is a gradient, so it sizes and animates with the views
/// around it.
final class GradientView: UIView {
    override class var layerClass: AnyClass { CAGradientLayer.self }

    // swiftlint:disable:next force_cast
    var gradientLayer: CAGradientLayer { layer as! CAGradientLayer }
}
