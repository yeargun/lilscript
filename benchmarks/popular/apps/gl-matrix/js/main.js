import * as root from "gl-matrix";

import {
  runMat2Contract,
  runMat2dContract,
  runMat3Contract,
  runMat4Contract,
  runCommonContract,
  runQuatContract,
  runQuat2Contract,
  runRootContract,
  runVec2Contract,
  runVec3Contract,
  runVec4Contract,
} from "../contract.js";

const { glMatrix, mat2, mat2d, mat3, mat4, quat, quat2, vec2, vec3, vec4 } = root;
runRootContract(root);
runCommonContract(glMatrix, vec2);
runVec2Contract(vec2);
runVec3Contract(vec3);
runVec4Contract(vec4);
runMat2Contract(mat2);
runMat2dContract(mat2d);
runMat3Contract(mat3);
runMat4Contract(mat4);
runQuatContract(quat);
runQuat2Contract(quat2);
