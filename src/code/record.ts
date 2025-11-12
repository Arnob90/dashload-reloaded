type DeepReadonly<T> = {
    readonly [K in keyof T]: T[K] extends object ? DeepReadonly<T[K]> : T[K];
};

class RecordMaker<RecordMakerObj> {
    readonly requiredObj: DeepReadonly<RecordMakerObj>;
    constructor(requiredObj: RecordMakerObj) {
        this.requiredObj = requiredObj;
    }
    with(givenObj: Partial<RecordMakerObj>) {
        return {
            ...this.requiredObj,
            ...givenObj,
        };
    }
}
