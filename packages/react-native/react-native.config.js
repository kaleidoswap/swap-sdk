module.exports = {
  dependency: {
    platforms: {
      android: {
        // Must match codegenConfig.outputDir.android's default
        // (`android/generated`) so React Native finds its generated JNI glue.
        cmakeListsPath: "generated/jni/CMakeLists.txt",
      },
    },
  },
};
