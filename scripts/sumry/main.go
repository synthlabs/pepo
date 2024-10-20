package main

import (
	"bufio"
	"bytes"
	"flag"
	"fmt"
	"html/template"
	"io"
	"os"
	"regexp"
	"time"

	"github.com/charmbracelet/log"
	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/plumbing"
	"github.com/go-git/go-git/v5/plumbing/object"
)

// this file is disgusting, I'm just prototyping :)

const (
	// this matches our commit format of {type}({scope}): {message}
	COMMIT_SUMRY_FMT_MATCHER = `(.*)\((.+)\):\s(.*)`
	COMMIT_MAGIC_STRING      = `^\[commit\]: # '(.+)'$`
	SUMRY_FILE_TMPL          = `[commit]: # '{{.Commit}}'

Features:

{{range .Features -}}
	- {{. -}}
{{end}}
Fixes:

{{range .Fixes -}}
	- {{. -}}
{{end}}
Misc:

{{range .Misc -}}
	- {{. -}}
{{end -}}
`
)

var (
	projectRoot     string
	sumryFile       string
	sumryArchiveDir string
	update          bool
	forceUpdate     bool
	debugLog        bool
)

type SumryFileTemplateVars struct {
	Commit   string
	Features []string
	Fixes    []string
	Misc     []string
}

// this one goes to you Rob
func parseSumryFileCommit(line string) (commit string, found bool) {
	regex := regexp.MustCompile(COMMIT_MAGIC_STRING)

	if match := regex.FindStringSubmatch(line); len(match) == 2 {
		commit = match[1]
		found = true
	}
	return
}

func headFile(fileName string) (string, error) {
	file, err := os.Open(fileName)
	if err != nil {
		return "", err
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	if scanner.Scan() {
		return scanner.Text(), nil
	} else if err := scanner.Err(); err != nil {
		return "", err
	}
	return "", nil
}

func commitsSinceHash(prevCommit string, head *plumbing.Reference, repo *git.Repository) ([]*object.Commit, error) {
	commits := []*object.Commit{}

	cIter, err := repo.Log(&git.LogOptions{From: head.Hash(), Order: git.LogOrderCommitterTime})
	if err != nil {
		return commits, err
	}

	for {
		c, err := cIter.Next()
		if err != nil && err != io.EOF {
			return commits, err
		}
		if c == nil {
			break
		}
		commits = append(commits, c)
		if c.Hash.String() == prevCommit {
			break
		}
	}
	return commits, nil
}

func mustGetRepo(gitPath string, logger *log.Logger) *git.Repository {
	repo, err := git.PlainOpen(gitPath)
	if err != nil {
		logger.Fatal("failed to open git repo", "path", gitPath, "error", err)
	}
	return repo
}

func generateSumryFileText(vars SumryFileTemplateVars) string {
	output := bytes.Buffer{}
	tmpl, err := template.New("sumry").Parse(SUMRY_FILE_TMPL)
	if err != nil {
		log.Error("failed to parse template", "error", err)
		return ""
	}
	if err = tmpl.Execute(&output, vars); err != nil {
		log.Error("failed to execute the template", "error", err)
	}
	return output.String()
}

func generateSumryTemplateVars(head string, commits []*object.Commit) SumryFileTemplateVars {
	vars := SumryFileTemplateVars{
		Commit:   head,
		Features: []string{},
		Fixes:    []string{},
		Misc:     []string{},
	}

	regex := regexp.MustCompile(COMMIT_SUMRY_FMT_MATCHER)
	for _, commit := range commits {
		matches := regex.FindStringSubmatch(commit.Message)
		if len(matches) == 4 {
			section := matches[1]
			msg := fmt.Sprintf("(%s): %s\n", matches[2], matches[3])

			switch section {
			case "feat":
				vars.Features = append(vars.Features, msg)
			case "fix":
				vars.Fixes = append(vars.Fixes, msg)
			default:
				vars.Misc = append(vars.Misc, msg)
			}
		} else {
			// it didn't match the sumry format, so just add it to misc
			vars.Misc = append(vars.Misc, commit.Message)
		}
	}

	return vars
}

func backupSumryFile() error {
	srcFile := fmt.Sprintf("%s/%s", projectRoot, sumryFile)
	dstFile := fmt.Sprintf("%s/%s/%s.%s", projectRoot, sumryArchiveDir, time.Now().Format("200601021504"), sumryFile)

	log.Debug("backing up sumry file", "src", srcFile, "dst", dstFile)

	return os.Rename(srcFile, dstFile)
}

func writeSumryFile(contents string) error {
	fileName := fmt.Sprintf("%s/%s", projectRoot, sumryFile)

	log.Debug("writing sumry file", "file", fileName)

	return os.WriteFile(fileName, []byte(contents), 0o664)
}

func init() {
	flag.StringVar(&projectRoot, "project-root", "", "the project root vergo will parse")
	flag.StringVar(&sumryFile, "sumry-file", "SUMRY.md", "the previous sumry output file (relative to the project root)")
	flag.StringVar(&sumryArchiveDir, "sumry-archive-dir", "archive", "the dir to move old sumry files (relative to the project root)")
	flag.BoolVar(&update, "update", false, "enable updating the sumry file, otherwise prints to stdout")
	flag.BoolVar(&forceUpdate, "force-update", false, "force updating the sumry file even if the commit hasn't changed")
	flag.BoolVar(&debugLog, "debug", false, "enable debug logs")

	flag.Parse()
}

func main() {
	if debugLog {
		log.SetLevel(log.DebugLevel)
	}

	log.Info("SUMRY")
	logger := log.With(
		"update", update,
		"force-update", forceUpdate,
	)
	logger.Debug("config",
		"sumry-file", sumryFile,
		"archive-dir", sumryArchiveDir,
		"project-root", projectRoot,
	)

	fileName := fmt.Sprintf("%s/%s", projectRoot, sumryFile)
	firstLine, err := headFile(fileName)
	if err != nil {
		log.Fatal("failed to read file", "file", fileName, "error", err)
	}

	if commit, ok := parseSumryFileCommit(firstLine); ok {
		logger.Info("Parsed previous log commit", "commit", commit)

		gitPath := fmt.Sprintf("%s/%s", projectRoot, ".git")
		repo := mustGetRepo(gitPath, logger)

		head, err := repo.Head()
		if err != nil {
			logger.Fatal("failed to get HEAD", "error", err)
		}
		logger.Debug("current HEAD", "ref", head.Hash())

		if commit == head.Hash().String() {
			logger.Warn("sumry file is already up-to-date. Use -force-update=true to skip this check")
			if !forceUpdate {
				return
			}
		}

		logger.Debug("getting commits")
		commits, err := commitsSinceHash(commit, head, repo)
		if err != nil {
			log.Fatal("failed to get commits since last update", "err", err)
		}

		logger.Debug("generating sumry vars")
		vars := generateSumryTemplateVars(head.Hash().String(), commits)
		logger.Debug("generating sumry file")
		fileContents := generateSumryFileText(vars)
		if !update {
			logger.Info("update not enabled, printing sumry file")
			// print the file to stdout so it's easy to capture in scripts if you want
			fmt.Fprint(os.Stdout, fileContents)
		} else {
			if err := backupSumryFile(); err != nil {
				logger.Fatal("failed to backup sumry file")
			}

			if err := writeSumryFile(fileContents); err != nil {
				logger.Fatal("failed to write new sumry file")
			}
		}
	} else {
		logger.Error("failed to locate previous sumry file")
	}
}
