package main

import (
	"flag"

	"github.com/charmbracelet/log"
)

var (
	projectRoot string
	majorBump   bool
	minorBump   bool
	patchBump   bool
	debugLog    bool
)

func init() {
	flag.StringVar(&projectRoot, "project-root", "", "the project root vergo will parse")
	flag.BoolVar(&majorBump, "major-bump", false, "bump the major version")
	flag.BoolVar(&minorBump, "minor-bump", false, "bump the minor version")
	flag.BoolVar(&patchBump, "patch-bump", true, "bump the patch version")
	flag.BoolVar(&debugLog, "debug", false, "enable debug logs")

	flag.Parse()
}

func main() {
	if debugLog {
		log.SetLevel(log.DebugLevel)
	}

	log.Info("VERGO")
	log.Debug("config",
		"project-root", projectRoot,
		"major-bump", majorBump,
		"minor-bump", minorBump,
		"patch-bump", patchBump,
	)
}
