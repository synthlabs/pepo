package main

import (
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/charmbracelet/log"
)

var (
	projectRoot string
	majorBump   bool
	minorBump   bool
	patchBump   bool
	debugLog    bool
	update      bool
)

func init() {
	flag.StringVar(&projectRoot, "project-root", "", "the project root vergo will parse")
	flag.BoolVar(&majorBump, "major-bump", false, "bump the major version")
	flag.BoolVar(&minorBump, "minor-bump", false, "bump the minor version")
	flag.BoolVar(&patchBump, "patch-bump", true, "bump the patch version")
	flag.BoolVar(&debugLog, "debug", false, "enable debug logs")
	flag.BoolVar(&update, "update", false, "actually update files")

	flag.Parse()
}

type NPMPackage struct {
	Name            string            `json:"name"`
	Version         string            `json:"version"`
	Private         bool              `json:"private"`
	Scripts         map[string]string `json:"scripts"`
	DevDependancies map[string]string `json:"devDependencies"`
	Type            string            `json:"type"`
	Dependancies    map[string]string `json:"dependencies"`
}

type Version struct {
	Major int
	Minor int
	Patch int
}

func (v Version) ToString() string {
	return fmt.Sprintf("%d.%d.%d", v.Major, v.Minor, v.Patch)
}

func (v Version) Bump(major, minor, patch bool) Version {
	if major {
		v.Major++
	}
	if minor {
		v.Minor++
	}
	if patch {
		v.Patch++
	}

	return v
}

func NewVersion(version string) (v Version, err error) {
	parts := strings.SplitN(version, ".", 3)

	if len(parts) != 3 {
		err = errors.New("unexpected version parts, expects <major.minor.patch> format")
		return
	}

	v.Major, err = strconv.Atoi(parts[0])
	if err != nil {
		return
	}
	v.Minor, err = strconv.Atoi(parts[1])
	if err != nil {
		return
	}
	v.Patch, err = strconv.Atoi(parts[2])
	if err != nil {
		return
	}

	return
}

func processPackageJson() {
	pkgJsonPath := fmt.Sprintf("%s/package.json", projectRoot)

	packageJsonBytes, err := os.ReadFile(pkgJsonPath)
	if err != nil {
		log.Fatal("failed to read file", "file", pkgJsonPath)
	}

	var pkgJson NPMPackage
	if err := json.Unmarshal(packageJsonBytes, &pkgJson); err != nil {
		log.Fatal("failed to parse package.json", "err", err)
	}

	log.Debug("parsing package.json version", "version", pkgJson.Version)
	origVersion, err := NewVersion(pkgJson.Version)
	if err != nil {
		log.Fatal("invalid package.json version format", "version", pkgJson.Version, "err", err)
	}

	log.Debug("parsed package.json version", "version", origVersion)

	version := origVersion.Bump(majorBump, minorBump, patchBump)

	log.Debug("package.json version bumped", "version", version)

	log.Info("package.json version updated", "before", origVersion.ToString(), "after", version.ToString())

	if !update {
		log.Info("update not enabled, exiting")
		return
	}

	pkgJson.Version = version.ToString()

	outData, err := json.MarshalIndent(pkgJson, "", "\t")
	if err != nil {
		log.Fatal("failed to marshal package.json", "err", err)
	}

	if err := os.WriteFile(pkgJsonPath, outData, 0o664); err != nil {
		log.Fatal("failed to write file", "file", pkgJsonPath, "err", err)
	}
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

	processPackageJson()
}