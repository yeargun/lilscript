import json, subprocess, tempfile
from pathlib import Path
binary='/home/azureuser/lilscript-work/bin/q2-decisions-1/lilscript'
with tempfile.TemporaryDirectory(prefix='lilscript-choices-cli-') as tmp:
    root=Path(tmp); project=root/'project'; project.mkdir()
    entry=project/'main.lil'; entry.write_text('export int answer(){return 17;}')
    config=project/'lilscript.toml'; config.write_text('[effort]\nlevel=1\n[decisions]\nread="missing.lock"\n')
    args=[binary,str(entry),'--config',str(config),'--target','js-module','--explain','json','-o',str(root/'out.mjs')]
    def run(extra):
        result=subprocess.run(args+extra,cwd=root,text=True,capture_output=True,check=True)
        return json.loads(result.stderr[result.stderr.index('{\n'):])
    saved=run(['--write-choices'])
    assert (root/'lilscript.choices.lock').is_file()
    assert saved['decisions']['write']=='written' and saved['decisions']['read']=='miss'
    replay=run(['--choices','lilscript.choices.lock'])
    assert replay['decisions']['read']=='matched'
    assert replay['search']['terminal']['objectives'][0]['starts'][0]['name']=='decision-lock'
    off=run(['--choices','off'])
    assert off.get('decisions') is None
    native=subprocess.run([binary,str(entry),'--config',str(config),'--target','c'],cwd=root,text=True,capture_output=True)
    assert native.returncode and 'decision locks require a JavaScript target' in native.stderr
    help=subprocess.run([binary,'--help'],text=True,capture_output=True,check=True).stdout
    assert '--choices' in help and '--write-choices' in help
    result={'default_write_in_current_directory':True,'explicit_read_overrides_toml':True,'off_disables_toml_read':True,'native_refuses_locks':True,'help_documents_flags':True}
Path(__file__).with_name('cli-controls.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result))
