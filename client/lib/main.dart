import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_circle_color_picker/flutter_circle_color_picker.dart';
import 'package:logger/web.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:quick_blue/quick_blue.dart';

void main() {
    runApp(const MyApp());
}

class MyApp extends StatefulWidget {
    const MyApp({super.key});

    @override State<MyApp> createState() => _MyAppState();
}

class _MyAppState extends State<MyApp> {
    final GlobalKey<NavigatorState> navigatorKey = GlobalKey<NavigatorState>();
    
    @override void initState() {
        super.initState();

        QuickBlue.setConnectionHandler((deviceId, connectionState) {
            switch (connectionState) {
                case BlueConnectionState.connected:
                    QuickBlue.discoverServices(deviceId);
                    navigatorKey.currentState!.pushReplacement(
                            MaterialPageRoute(builder: (context) => ConnectedScreen(deviceId))
                    );
                case BlueConnectionState.disconnected:
                    navigatorKey.currentState!.pushReplacement(
                            MaterialPageRoute(builder: (context) => const LoadingScreen())
                    );
            }
        });
    }

    @override void dispose() {
        QuickBlue.stopScan();
        super.dispose();
    }

    @override Widget build(BuildContext context) {
        return MaterialApp(
            title: 'Flutter Demo',
            navigatorKey: navigatorKey,
            theme: ThemeData(
                colorScheme: ColorScheme.fromSeed(
                    seedColor: Colors.deepPurple.shade700,
                    brightness: Brightness.dark,
                ),
                useMaterial3: true,
            ),
            
            home: const LoadingScreen(),
            // home: const LoadingScreen(title: 'Mansion Lighting Control'),
        );
    }
}

class ConnectedScreen extends StatefulWidget {
    const ConnectedScreen(this.deviceId, {super.key});
    final String deviceId;

  @override
  State<ConnectedScreen> createState() => _ConnectedScreenState();
}

class _ConnectedScreenState extends State<ConnectedScreen> {
    double _skipValue = 0;
    double _brightnessValue = 0;

    final CircleColorPickerController _controller = CircleColorPickerController();

    static const serviceId =  "6d696368-6165-6c73-206d-616e73696f6e";
    static const baseColorId = "62617365-2063-6f6c-6f72-000000000000";
    static const brightnessId = "62726967-6874-6e65-7373-000000000000";
    static const skipId = "736b6970-0000-0000-0000-000000000000";

    @override void dispose() {
        _controller.dispose();
        super.dispose();
      }

    @override Widget build(BuildContext context) {
        return Scaffold(
            appBar: AppBar(
                backgroundColor: Theme.of(context).colorScheme.inversePrimary,
                title: const Text('Mansion Lighting Control'),
            ),
            body: Center(
                child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                        CircleColorPicker(
                            textStyle: const TextStyle(color: Colors.transparent, fontSize: 0),
                            controller: _controller,
                            onChanged: (color) { 
                                setState(() {}); // force redraw
                                QuickBlue.writeValue(widget.deviceId, serviceId, baseColorId, Uint8List.fromList([color.red, color.green, color.blue]), BleOutputProperty.withResponse);
                            },
                        ),
                        Slider( // Brightness Slider
                            value: _brightnessValue, 
                            max: 99,
                            onChanged: (v) => setState(() { 
                                _brightnessValue = v; 
                                var brightness = (v + pow(v, 2.5)) / 400;
                                brightness = (v / 100) * 255;
                                brightness = exp(v * 0.05545) - 1;
                                QuickBlue.writeValue(
                                    widget.deviceId, 
                                    serviceId, 
                                    brightnessId,
                                    Uint8List.fromList([brightness.toInt()]), 
                                    BleOutputProperty.withResponse
                                );
                            }),
                        ),
                        Slider( // Skip Slider
                            value: _skipValue, 
                            max: 10,
                            divisions: 10,
                            onChanged: (v) => setState(() { 
                                _skipValue = v; 
                                QuickBlue.writeValue(widget.deviceId, serviceId, skipId, Uint8List.fromList([v.toInt()]), BleOutputProperty.withResponse);
                            }),
                        ),
                    ],
                ),
            ),
        );
    }
}

class LoadingScreen extends StatefulWidget {
    const LoadingScreen({super.key});

  @override
  State<LoadingScreen> createState() => _LoadingScreenState();
}

class _LoadingScreenState extends State<LoadingScreen> {
    @override void initState() {
        super.initState();

        // Bluetooth Scanning
        QuickBlue.scanResultStream.listen((result) {
            if (result.name.startsWith("mansion lighting")) {
                Logger().d('found device: ${result.name}');
                // TODO: do something
                QuickBlue.connect(result.deviceId);
            }
        });
        
        if (Platform.isAndroid) {
            Permission.bluetooth.request().then((status) { 
                // TODO: handle rejection
                QuickBlue.startScan();
            });
        } else {
            QuickBlue.startScan();
        }
    }

    @override Widget build(BuildContext context) {
        return Scaffold(
            appBar: AppBar(
                backgroundColor: Theme.of(context).colorScheme.inversePrimary,
                title: const Text('Mansion Lighting Control'),
            ),
            body: Container(
                constraints: const BoxConstraints.expand(),
                child: const Stack(
                    children: <Widget>[
                        Positioned.fill(
                            top: -175,
                            child: Align(
                                alignment: Alignment.center,
                                child: Text(
                                    'Searching for mansion lighting',
                                    style: TextStyle(fontSize: 24),
                                ),
                            ),
                        ),
                        Positioned.fill(
                            top: -50,
                            child: Align(
                                alignment: Alignment.center,
                                child: CircularProgressIndicator(),
                            ),
                        ),
                    ],
                ),
            ),
        );
    }

    @override void dispose() {
        super.dispose();
        QuickBlue.stopScan();
    }
}
