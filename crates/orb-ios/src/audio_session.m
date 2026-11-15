//! The two things UIKit needs that winit 0.30 does not do for us: an active audio session, and a
//! scene delegate. iPadOS 26 and later kill an app that has not adopted the UIScene lifecycle
//! (`UIApplicationEvaluateRuntimeIssueForNoSceneLifecycleAdoption`, a brk), and a window that
//! belongs to no scene is never shown; winit creates plain UIWindows.
#import <AVFoundation/AVFoundation.h>
#import <UIKit/UIKit.h>
#import <objc/runtime.h>

static void orb_attach(UIWindowScene *scene) {
    if (scene == nil) {
        return;
    }
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    for (UIWindow *window in UIApplication.sharedApplication.windows) {
        if (window.windowScene == nil) {
            window.windowScene = scene;
        }
    }
#pragma clang diagnostic pop
}

static UIWindowScene *orb_scene(void) {
    for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
        if ([scene isKindOfClass:[UIWindowScene class]]) {
            return (UIWindowScene *)scene;
        }
    }
    return nil;
}

// winit makes its window with `-[UIWindow makeKeyAndVisible]` and then reads the size of the
// view; a window in no scene has a zero-sized frame in screen space, which wgpu turns into a
// zero-sized surface. The window joins the scene before it is made key.
static void (*orb_make_key_and_visible)(id, SEL);

static void orb_attach_and_make_key(UIWindow *window, SEL selector) {
    if (window.windowScene == nil) {
        window.windowScene = orb_scene();
    }
    orb_make_key_and_visible(window, selector);
}

__attribute__((constructor)) static void orb_install_scene_attachment(void) {
    Method method = class_getInstanceMethod([UIWindow class], @selector(makeKeyAndVisible));
    orb_make_key_and_visible = (void *)method_getImplementation(method);
    method_setImplementation(method, (IMP)orb_attach_and_make_key);
}

void orb_ios_audio_session(void) {
    AVAudioSession *session = [AVAudioSession sharedInstance];
    NSError *error = nil;
    if (![session setCategory:AVAudioSessionCategoryPlayback error:&error]) {
        NSLog(@"orb-ios: audio session category: %@", error);
    }
    error = nil;
    if (![session setPreferredSampleRate:44100 error:&error]) {
        NSLog(@"orb-ios: audio session sample rate: %@", error);
    }
    error = nil;
    if (![session setPreferredIOBufferDuration:0.0232 error:&error]) {
        NSLog(@"orb-ios: audio session buffer: %@", error);
    }
    error = nil;
    if (![session setActive:YES error:&error]) {
        NSLog(@"orb-ios: audio session activation: %@", error);
    }
}

/// Attaches the windows winit has made so far to the app's window scene. Rust calls it once the
/// panel exists; the scene delegate calls it when the scene connects.
void orb_ios_attach_scene(void) {
    for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
        if ([scene isKindOfClass:[UIWindowScene class]]) {
            orb_attach((UIWindowScene *)scene);
            return;
        }
    }
}

/// The delegate `Info.plist` names: it exists to adopt the scene lifecycle, not to own a window.
@interface OrbSceneDelegate : UIResponder <UIWindowSceneDelegate>
@end

@implementation OrbSceneDelegate

- (void)scene:(UIScene *)scene
    willConnectToSession:(UISceneSession *)session
                 options:(UISceneConnectionOptions *)connectionOptions {
    if ([scene isKindOfClass:[UIWindowScene class]]) {
        orb_attach((UIWindowScene *)scene);
    }
}

- (void)sceneDidBecomeActive:(UIScene *)scene {
    if ([scene isKindOfClass:[UIWindowScene class]]) {
        orb_attach((UIWindowScene *)scene);
    }
}

@end
