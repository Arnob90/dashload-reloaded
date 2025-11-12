import { useState, useMemo } from "react";

export type ModeName = "Normal" | "Segmented" | "Smart";

export interface NormalMode {
    type: "Normal";
}

export interface SegmentMode {
    type: "Segmented";
    segmentNumber: number;
}

export interface SmartMode {
    type: "Smart";
}

type Mode = NormalMode | SegmentMode | SmartMode;

export interface ModeSelectorApi {
    mode: Mode;
    prevMode: Mode;
    setMode: (givenMode: ModeName) => void;
    setModeInfo: (toChangeTo: Mode) => void;
}

export function useDownloadModeManager(): ModeSelectorApi {
    const [normalOption, setNormalOption] = useState<NormalMode>({
        type: "Normal",
    });
    const [segmentOption, setSegmentOption] = useState<SegmentMode>({
        type: "Segmented",
        segmentNumber: 3,
    });
    const [smartOption, setSmartOption] = useState<SmartMode>({
        type: "Smart",
    });

    const [prevModeName, setPrevModeName] = useState<ModeName>("Normal");
    const [currentModeName, setCurrentModeName] = useState<ModeName>("Smart");

    const getModeAndSetter = (name: ModeName) => {
        switch (name) {
            case "Normal":
                return [normalOption, setNormalOption] as const;
            case "Segmented":
                return [segmentOption, setSegmentOption] as const;
            case "Smart":
                return [smartOption, setSmartOption] as const;
        }
    };

    const currentMode = useMemo(
        () => getModeAndSetter(currentModeName)[0],
        [currentModeName, normalOption, segmentOption, smartOption]
    );

    const prevMode = useMemo(
        () => getModeAndSetter(prevModeName)[0],
        [prevModeName, normalOption, segmentOption, smartOption]
    );

    const setMode = (givenMode: ModeName) => {
        setPrevModeName(currentModeName);
        setCurrentModeName(givenMode);
    };

    const setModeInfo = (toChangeTo: Mode) => {
        if (toChangeTo.type === currentModeName) {
            const [, setMode] = getModeAndSetter(toChangeTo.type);
            setMode(toChangeTo as any);
        }
    };

    return { mode: currentMode, prevMode, setMode, setModeInfo };
}
