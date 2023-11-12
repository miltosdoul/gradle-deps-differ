# Gradle dependencies differ

A Gradle 'dependencies' task output diffing tool. Outputs an HTML report or a JSON object. \
Useful for tracking down changed transitive dependencies when upgrading a library (e.g. Spring) which might impact the application.


## Usage

To build: \
`cargo build --release`

The program takes two command-line arguments `-b/--file-before` and `-a/--file-after` specifying the path 
of the two files to diff.

To output an HTML report with all the dependencies and the changes in versions, simply run: \
`gradle-deps-differ -b path/to/file1 -a path/to/file2`

To get the output of the `dependencies` task from your Gradle project, run: \
`./gradlew dependencies > dependencies.txt`


## JSON Output

To output the parsed dependencies and changes of versions as JSON instead, add the `--json` option: \
`gradle-deps-differ --json -b path/to/file1 -a path/to/file2`


The structure of the JSON output is as follows:
```json
[
  {
    "dependency": {
      "name": "jakarta.xml.bind-api",
      "namespace": "jakarta.xml.bind",
      "gradle_entries_before": [
        {
          "gradle_config_name": "compileClasspath",
          "versions": {
            "transitive": [
              "2.3.2"
            ],
            "pinned": "4.0.0"
          }
        },
        {
          "gradle_config_name": "productionRuntimeClasspath",
          "versions": {
            "transitive": [
              "3.0.1",
              "4.0.0",
              "2.3.2"
            ],
            "pinned": "4.0.0"
          }
        },
        ...
      ],
      "gradle_entries_after": [
        {
          "gradle_config_name": "compileClasspath",
          "versions": {
            "transitive": [
              "2.3.3",
              "2.3.2"
            ],
            "pinned": "4.0.1"
          }
        },
        {
          "gradle_config_name": "productionRuntimeClasspath",
          "versions": {
            "transitive": [
              "4.0.0",
              "2.3.3",
              "2.3.2"
            ],
            "pinned": "4.0.1"
          }
        },
        ...
      ]
    },
    "changed": true,
    "gradle_versions": [
      {
        "gradle_config_name": "compileClasspath",
        "version_before": "4.0.0",
        "version_after": "4.0.1"
      },
      {
        "gradle_config_name": "productionRuntimeClasspath",
        "version_before": "4.0.0",
        "version_after": "4.0.1"
      },
      ...
    ]
  },
  ...
]
```

where `transitive` signifies the versions of this dependency coming from other dependencies and `pinned` signifies the version which has been pinned for a particular Gradle task.

The `changed` field indicates whether the final version before and the final version after are different.


## Version resolution

For a given dependency, the final version of that in a given Gradle task is resolved in the below way:
* If the pinned version is specified, that is the final version regardless of if there are any transitive versions encountered.
* If the pinned version is not specified, the final version will be the greatest transitive encountered in that task block.


## Tests
To run the unit tests: \
`cargo test`


## License
Licensed under the [MIT license](https://opensource.org/license/mit/).